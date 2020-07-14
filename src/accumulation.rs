// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{AccountAdded, AccountId, AccumulationEvent, RewardsAccumulated, RewardsClaimed};
use safe_nd::{Error, Money, Result, RewardCounter, Work};
use std::collections::{HashMap, HashSet};

/// The book keeping of rewards.
/// The business rule is that a piece of data
/// is only rewarded once.
#[derive(Clone)]
pub struct Accumulation {
    idempotency: HashSet<Id>,
    accumulated: HashMap<AccountId, RewardCounter>,
}

/// Identification type
pub type Id = Vec<u8>;

impl Accumulation {
    /// ctor
    pub fn new(idempotency: HashSet<Id>, accumulated: HashMap<AccountId, RewardCounter>) -> Self {
        Self {
            idempotency,
            accumulated,
        }
    }

    /// -----------------------------------------------------------------
    /// ---------------------- Queries ----------------------------------
    /// -----------------------------------------------------------------

    ///
    pub fn get(&self, account: &AccountId) -> Option<&RewardCounter> {
        self.accumulated.get(account)
    }

    ///
    pub fn get_all(&self) -> &HashMap<AccountId, RewardCounter> {
        &self.accumulated
    }

    /// -----------------------------------------------------------------
    /// ---------------------- Cmds -------------------------------------
    /// -----------------------------------------------------------------

    pub fn add_account(&self, id: AccountId, work: Work) -> Result<AccountAdded> {
        if self.accumulated.contains_key(&id) {
            return Err(Error::BalanceExists);
        }
        Ok(AccountAdded { id, work })
    }

    ///
    pub fn accumulate(
        &self,
        id: Id,
        distribution: HashMap<AccountId, Money>,
    ) -> Result<RewardsAccumulated> {
        if self.idempotency.contains(&id) {
            return Err(Error::DataExists);
        }
        for (id, amount) in &distribution {
            if let Some(existing) = self.accumulated.get(&id) {
                if existing.add(*amount).is_none() {
                    return Err(Error::ExcessiveValue);
                }
            };
        }

        Ok(RewardsAccumulated { id, distribution })
    }

    ///
    pub fn claim(&self, account: AccountId) -> Result<RewardsClaimed> {
        let result = self.accumulated.get(&account);
        match result {
            None => Err(Error::NoSuchKey),
            Some(rewards) => Ok(RewardsClaimed {
                account,
                rewards: rewards.clone(),
            }),
        }
    }

    /// -----------------------------------------------------------------
    /// ---------------------- Mutation ---------------------------------
    /// -----------------------------------------------------------------

    /// Mutates state.
    pub fn apply(&mut self, event: AccumulationEvent) {
        use AccumulationEvent::*;
        match event {
            AccountAdded(e) => {
                let _ = self.accumulated.insert(
                    e.id,
                    RewardCounter {
                        reward: Money::zero(),
                        work: e.work,
                    },
                );
            }
            RewardsAccumulated(e) => {
                for (id, amount) in e.distribution {
                    let existing = match self.accumulated.get(&id) {
                        None => Default::default(),
                        Some(acc) => acc.clone(),
                    };
                    let accumulated = existing.add(amount).unwrap(); // this is OK, since validation shall happen before creating the event
                    let _ = self.idempotency.insert(e.id.clone());
                    let _ = self.accumulated.insert(id, accumulated);
                }
            }
            RewardsClaimed(e) => {
                let _ = self.accumulated.remove(&e.account);
            }
        }
    }
}
#[cfg(test)]
mod test {
    use super::{Accumulation, AccumulationEvent};
    use safe_nd::{Error, Money, PublicKey};
    use threshold_crypto::SecretKey;

    macro_rules! hashmap {
        ($( $key: expr => $val: expr ),*) => {{
             let mut map = ::std::collections::HashMap::new();
             $( let _ = map.insert($key, $val); )*
             map
        }}
    }

    #[test]
    fn when_data_was_not_previously_rewarded_reward_accumulates() -> Result<(), Error> {
        // --- Arrange ---
        let mut acc = Accumulation::new(Default::default(), Default::default());
        let account = get_random_pk();
        let data_hash = vec![1, 2, 3];
        let reward = Money::from_nano(10);
        let distribution = hashmap![account => reward];

        // --- Act ---
        // Try accumulate.
        let e = acc.accumulate(data_hash, distribution)?;

        // --- Assert ---
        // Confirm valid ..
        assert!(e.distribution.len() == 1);
        assert!(e.distribution.contains_key(&account));
        assert_eq!(&reward, e.distribution.get(&account).unwrap());
        acc.apply(AccumulationEvent::RewardsAccumulated(e));
        // .. and successful.
        if let Some(accumulated) = acc.get(&account) {
            assert_eq!(accumulated.reward, reward);
        }
        Ok(())
    }

    #[test]
    fn when_data_is_already_rewarded_accumulation_is_rejected() -> Result<(), Error>{
        // --- Arrange ---
        let mut acc = Accumulation::new(Default::default(), Default::default());
        let account = get_random_pk();
        let data_hash = vec![1, 2, 3];
        let reward = Money::from_nano(10);
        let distribution = hashmap![account => reward];

        // Accumulate reward.
        let reward = acc
            .accumulate(data_hash.clone(), distribution.clone())?;
        acc.apply(AccumulationEvent::RewardsAccumulated(reward));

        // --- Act ---
        // Try same data hash again ..

        // --- Assert ---
        // .. confirm not successful.
        assert_eq!(acc.accumulate(data_hash, distribution), Err(Error::DataExists));
        Ok(())
    }

    #[test]
    fn when_account_has_reward_it_can_claim() -> Result<(), Error>{
        // --- Arrange ---
        let mut acc = Accumulation::new(Default::default(), Default::default());
        let account = get_random_pk();
        let data_hash = vec![1, 2, 3];
        let reward = Money::from_nano(10);
        let distribution = hashmap![account => reward];
        let accumulation = acc.accumulate(data_hash, distribution)?;
        acc.apply(AccumulationEvent::RewardsAccumulated(accumulation));

        // --- Act + Assert ---
        // Try claim, confirm account and amount is correct.
        let e = acc.claim(account)?;
                assert!(e.account == account);
                assert!(e.rewards.reward == reward);
                acc.apply(AccumulationEvent::RewardsClaimed(e));
                Ok(())
    }

    #[test]
    fn when_reward_was_claimed_it_can_not_be_claimed_again() {
        // --- Arrange ---
        let mut acc = Accumulation::new(Default::default(), Default::default());
        let account = get_random_pk();
        let data_hash = vec![1, 2, 3];
        let reward = Money::from_nano(10);
        let distribution = hashmap![account => reward];

        let accumulation = acc.accumulate(data_hash, distribution).unwrap();
        acc.apply(AccumulationEvent::RewardsAccumulated(accumulation));

        // Claim the account reward.
        let claim = acc.claim(account).unwrap();
        acc.apply(AccumulationEvent::RewardsClaimed(claim));

        // --- Act ---
        // Try claim the account reward again ..
        let result = acc.claim(account);

        // --- Assert ---
        // .. confirm not successful.
        match result {
            Ok(_) => assert!(false),
            Err(err) => assert_eq!(err, Error::NoSuchKey),
        }
    }

    #[test]
    fn when_account_has_no_reward_it_can_not_claim() {
        // --- Arrange ---
        let acc = Accumulation::new(Default::default(), Default::default());
        let account = get_random_pk();

        // --- Act + Assert ---
        // Try claim the account reward again, confirm not successful.
        let result = acc.claim(account);
        match result {
            Ok(_) => assert!(false),
            Err(err) => assert_eq!(err, Error::NoSuchKey),
        }
    }

    #[test]
    fn when_reward_was_claimed_get_returns_none() {
        // --- Arrange ---
        let mut acc = Accumulation::new(Default::default(), Default::default());
        let account = get_random_pk();
        let data_hash = vec![1, 2, 3];
        let reward = Money::from_nano(10);
        let distribution = hashmap![account => reward];
        let accumulation = acc.accumulate(data_hash, distribution).unwrap();
        acc.apply(AccumulationEvent::RewardsAccumulated(accumulation));
        let claim = acc.claim(account).unwrap();
        acc.apply(AccumulationEvent::RewardsClaimed(claim));

        // --- Act ---
        // Try get the account reward.
        let result = acc.get(&account);

        // --- Assert ---
        assert!(result.is_none());
    }

    fn get_random_pk() -> PublicKey {
        PublicKey::from(SecretKey::random().public_key())
    }
}
