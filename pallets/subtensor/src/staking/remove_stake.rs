use super::*;

impl<T: Config> Pallet<T> {
    /// ---- The implementation for the extrinsic remove_stake: Removes stake from a hotkey account and adds it onto a coldkey.
    ///
    /// # Args:
    /// * 'origin': (<T as frame_system::Config>RuntimeOrigin):
    ///     -  The signature of the caller's coldkey.
    ///
    /// * 'hotkey' (T::AccountId):
    ///     -  The associated hotkey account.
    ///
    /// * 'stake_to_be_added' (u64):
    ///     -  The amount of stake to be added to the hotkey staking account.
    ///
    /// # Event:
    /// * StakeRemoved;
    ///     -  On the successfully removing stake from the hotkey account.
    ///
    /// # Raises:
    /// * 'NotRegistered':
    ///     -  Thrown if the account we are attempting to unstake from is non existent.
    ///
    /// * 'NonAssociatedColdKey':
    ///     -  Thrown if the coldkey does not own the hotkey we are unstaking from.
    ///
    /// * 'NotEnoughStakeToWithdraw':
    ///     -  Thrown if there is not enough stake on the hotkey to withdwraw this amount.
    ///
    /// * 'TxRateLimitExceeded':
    ///     -  Thrown if key has hit transaction rate limit
    ///
    pub fn do_remove_stake(
        origin: T::RuntimeOrigin,
        hotkey: T::AccountId,
        stake_to_be_removed: u64,
    ) -> dispatch::DispatchResult {
        // We check the transaction is signed by the caller and retrieve the T::AccountId coldkey information.
        let coldkey = ensure_signed(origin)?;
        log::debug!(
            "do_remove_stake( origin:{:?} hotkey:{:?}, stake_to_be_removed:{:?} )",
            coldkey,
            hotkey,
            stake_to_be_removed
        );

        // Ensure that the hotkey account exists this is only possible through registration.
        ensure!(
            Self::hotkey_account_exists(&hotkey),
            Error::<T>::HotKeyAccountNotExists
        );

        // Ensure that the hotkey allows delegation or that the hotkey is owned by the calling coldkey.
        ensure!(
            Self::hotkey_is_delegate(&hotkey) || Self::coldkey_owns_hotkey(&coldkey, &hotkey),
            Error::<T>::HotKeyNotDelegateAndSignerNotOwnHotKey
        );

        // Ensure that the stake amount to be removed is above zero.
        ensure!(stake_to_be_removed > 0, Error::<T>::StakeToWithdrawIsZero);

        // Ensure that the hotkey has enough stake to withdraw.
        ensure!(
            Self::has_enough_stake(&coldkey, &hotkey, stake_to_be_removed),
            Error::<T>::NotEnoughStakeToWithdraw
        );

        // Opt-in lock: stake added via add_stake_locked cannot be removed
        // before its unlock block. Free stake above the locked amount is always
        // withdrawable. The funds never leave the staker; the chain only
        // refuses early withdrawal.
        let (locked_amount, unlock_block) = StakeLock::<T>::get(&coldkey, &hotkey);
        if locked_amount > 0 {
            let now = Self::get_current_block_as_u64();
            if now < unlock_block {
                let current = Self::get_stake_for_coldkey_and_hotkey(&coldkey, &hotkey);
                let free = current.saturating_sub(locked_amount);
                ensure!(stake_to_be_removed <= free, Error::<T>::StakeStillLocked);
            } else {
                // Lock expired: clear it so the map does not grow forever.
                StakeLock::<T>::remove(&coldkey, &hotkey);
            }
        }

        Self::try_increase_staking_counter(&coldkey, &hotkey)?;

        // We remove the balance from the hotkey.
        Self::decrease_stake_on_coldkey_hotkey_account(&coldkey, &hotkey, stake_to_be_removed);

        // Track this removal in the stake delta.
        StakeDeltaSinceLastEmissionDrain::<T>::mutate(&hotkey, &coldkey, |stake_delta| {
            *stake_delta = stake_delta.saturating_sub_unsigned(stake_to_be_removed as u128);
        });

        // We add the balance to the coldkey.  If the above fails we will not credit this coldkey.
        Self::add_balance_to_coldkey_account(&coldkey, stake_to_be_removed);

        // If the stake is below the minimum, we clear the nomination from storage.
        // This only applies to nominator stakes.
        // If the coldkey does not own the hotkey, it's a nominator stake.
        let new_stake = Self::get_stake_for_coldkey_and_hotkey(&coldkey, &hotkey);
        Self::clear_small_nomination_if_required(&hotkey, &coldkey, new_stake);

        // Check if stake lowered below MinStake and remove Pending children if it did
        if Self::get_total_stake_for_hotkey(&hotkey) < StakeThreshold::<T>::get() {
            Self::get_all_subnet_netuids().iter().for_each(|netuid| {
                PendingChildKeys::<T>::remove(netuid, &hotkey);
            })
        }

        // Set last block for rate limiting
        let block = Self::get_current_block_as_u64();
        Self::set_last_tx_block(&coldkey, block);

        // Emit the unstaking event.
        log::debug!(
            "StakeRemoved( hotkey:{:?}, stake_to_be_removed:{:?} )",
            hotkey,
            stake_to_be_removed
        );
        Self::deposit_event(Event::StakeRemoved(hotkey, stake_to_be_removed));

        // Done and ok.
        Ok(())
    }

    /// Return a matured locked stake to its owner's free balance and clear the
    /// lock. Permissionless on purpose: the funds always go to `coldkey`, never
    /// to whoever calls, so anyone may settle a lock that has reached its term
    /// (a daily off-chain job does, since the runtime cannot schedule itself
    /// cheaply). A lock that is absent or not yet at its unlock block is
    /// refused, so this can never pull stake out early.
    ///
    /// The WHOLE position on the pair is returned, principal and the dividends
    /// it earned: the point of the term is that the money then leaves staking
    /// instead of quietly continuing to earn.
    pub fn do_unlock_matured_stake(
        origin: T::RuntimeOrigin,
        coldkey: T::AccountId,
        hotkey: T::AccountId,
    ) -> dispatch::DispatchResult {
        let _who = ensure_signed(origin)?;

        let (locked_amount, unlock_block) = StakeLock::<T>::get(&coldkey, &hotkey);
        ensure!(locked_amount > 0, Error::<T>::StakeNotMatured);
        let now = Self::get_current_block_as_u64();
        ensure!(now >= unlock_block, Error::<T>::StakeNotMatured);

        // Clear the lock first so a repeated call is a harmless no-op.
        StakeLock::<T>::remove(&coldkey, &hotkey);

        let amount = Self::get_stake_for_coldkey_and_hotkey(&coldkey, &hotkey);
        if amount > 0 {
            // Same steps do_remove_stake takes, for the target coldkey. The
            // per-account rate limit is intentionally skipped: this is a
            // system-triggered settlement, not a user spamming withdrawals.
            Self::decrease_stake_on_coldkey_hotkey_account(&coldkey, &hotkey, amount);
            StakeDeltaSinceLastEmissionDrain::<T>::mutate(&hotkey, &coldkey, |stake_delta| {
                *stake_delta = stake_delta.saturating_sub_unsigned(amount as u128);
            });
            Self::add_balance_to_coldkey_account(&coldkey, amount);

            let new_stake = Self::get_stake_for_coldkey_and_hotkey(&coldkey, &hotkey);
            Self::clear_small_nomination_if_required(&hotkey, &coldkey, new_stake);

            if Self::get_total_stake_for_hotkey(&hotkey) < StakeThreshold::<T>::get() {
                Self::get_all_subnet_netuids().iter().for_each(|netuid| {
                    PendingChildKeys::<T>::remove(netuid, &hotkey);
                })
            }
        }

        Self::deposit_event(Event::StakeRemoved(hotkey, amount));
        Ok(())
    }
}
