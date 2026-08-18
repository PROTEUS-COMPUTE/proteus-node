use super::*;

/// A lock shorter than a day is not a commitment, and one longer than ten years
/// is a typo. Both bounds are in blocks, at 12 seconds per block.
pub const MIN_LOCK_BLOCKS: u64 = 7_200;
pub const MAX_LOCK_BLOCKS: u64 = 26_280_000;

impl<T: Config> Pallet<T> {
    /// ---- The implementation for the extrinsic add_stake: Adds stake to a hotkey account.
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
    /// * StakeAdded;
    ///     -  On the successfully adding stake to a global account.
    ///
    /// # Raises:
    /// * 'NotEnoughBalanceToStake':
    ///     -  Not enough balance on the coldkey to add onto the global account.
    ///
    /// * 'NonAssociatedColdKey':
    ///     -  The calling coldkey is not associated with this hotkey.
    ///
    /// * 'BalanceWithdrawalError':
    ///     -  Errors stemming from transaction pallet.
    ///
    /// * 'TxRateLimitExceeded':
    ///     -  Thrown if key has hit transaction rate limit
    ///
    pub fn do_add_stake(
        origin: T::RuntimeOrigin,
        hotkey: T::AccountId,
        stake_to_be_added: u64,
    ) -> dispatch::DispatchResult {
        // We check that the transaction is signed by the caller and retrieve the T::AccountId coldkey information.
        let coldkey = ensure_signed(origin)?;
        log::debug!(
            "do_add_stake( origin:{:?} hotkey:{:?}, stake_to_be_added:{:?} )",
            coldkey,
            hotkey,
            stake_to_be_added
        );

        // Ensure the callers coldkey has enough stake to perform the transaction.
        ensure!(
            Self::can_remove_balance_from_coldkey_account(&coldkey, stake_to_be_added),
            Error::<T>::NotEnoughBalanceToStake
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

        // If coldkey is not owner of the hotkey, it's a nomination stake.
        if !Self::coldkey_owns_hotkey(&coldkey, &hotkey) {
            let total_stake_after_add =
                Stake::<T>::get(&hotkey, &coldkey).saturating_add(stake_to_be_added);

            ensure!(
                total_stake_after_add >= NominatorMinRequiredStake::<T>::get(),
                Error::<T>::NomStakeBelowMinimumThreshold
            );
        }

        Self::try_increase_staking_counter(&coldkey, &hotkey)?;

        // Ensure the remove operation from the coldkey is a success.
        let actual_amount_to_stake =
            Self::remove_balance_from_coldkey_account(&coldkey, stake_to_be_added)?;

        // If we reach here, add the balance to the hotkey.
        Self::increase_stake_on_coldkey_hotkey_account(&coldkey, &hotkey, actual_amount_to_stake);

        // Track this addition in the stake delta.
        StakeDeltaSinceLastEmissionDrain::<T>::mutate(&hotkey, &coldkey, |stake_delta| {
            *stake_delta = stake_delta.saturating_add_unsigned(stake_to_be_added as u128);
        });

        // Set last block for rate limiting
        let block = Self::get_current_block_as_u64();
        Self::set_last_tx_block(&coldkey, block);

        log::debug!(
            "StakeAdded( hotkey:{:?}, stake_to_be_added:{:?} )",
            hotkey,
            actual_amount_to_stake
        );
        Self::deposit_event(Event::StakeAdded(hotkey, actual_amount_to_stake));

        // Ok and return.
        Ok(())
    }

    /// Add stake AND lock it for `lock_blocks` blocks. The stake is added
    /// exactly like do_add_stake; on top, the amount actually staked is
    /// recorded as locked until `now + lock_blocks`, and remove_stake refuses
    /// to withdraw the locked amount before then. Adding more locked stake sums
    /// the amounts and keeps the LATEST unlock block. Nothing is held by us.
    pub fn do_add_stake_locked(
        origin: T::RuntimeOrigin,
        hotkey: T::AccountId,
        stake_to_be_added: u64,
        lock_blocks: u64,
    ) -> dispatch::DispatchResult {
        let coldkey = ensure_signed(origin.clone())?;
        // A zero-block lock would be already matured on creation, which is a
        // standing permission for anyone to settle the position rather than a
        // commitment. The upper bound keeps a typo from locking stake for
        // longer than the chain is likely to exist.
        ensure!(
            lock_blocks >= MIN_LOCK_BLOCKS && lock_blocks <= MAX_LOCK_BLOCKS,
            Error::<T>::InvalidLockDuration
        );
        // Measure the real increase: do_add_stake may stake less than asked.
        let before = Self::get_stake_for_coldkey_and_hotkey(&coldkey, &hotkey);
        Self::do_add_stake(origin, hotkey.clone(), stake_to_be_added)?;
        let after = Self::get_stake_for_coldkey_and_hotkey(&coldkey, &hotkey);
        let added = after.saturating_sub(before);
        let now = Self::get_current_block_as_u64();
        let unlock = now.saturating_add(lock_blocks);
        StakeLock::<T>::mutate(&coldkey, &hotkey, |lock| {
            lock.0 = lock.0.saturating_add(added);
            if unlock > lock.1 {
                lock.1 = unlock;
            }
        });
        Ok(())
    }
}
