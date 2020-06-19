// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Work;
use rand::distributions::{uniform::Uniform as Range, Distribution};
use safe_nd::{AccountId, Money};
use std::collections::HashMap;
use std::hash::Hash;

/// This algo allows for setting a base cost together with a
/// cost proportional to some work, as measured by a minimum work unit.

pub trait RewardAlgo {
    fn set(&mut self, base_cost: Money);
    fn work_cost(&self, work_units: usize) -> Money;
    fn total_reward(&self, factor: f64, work_cost: Money) -> Money;
    fn distribute(
        &self,
        total_reward: Money,
        accounts_work: HashMap<AccountId, Work>,
    ) -> HashMap<AccountId, Money>;
}

/// Cost of, and rewards for, storage.
pub struct StorageRewards {
    base_cost: Money,
}

impl StorageRewards {
    pub fn new(base_cost: Money) -> Self {
        Self { base_cost }
    }
}

impl RewardAlgo for StorageRewards {
    /// Use this to update the base cost,
    /// as per any desired formula and frequency.
    fn set(&mut self, base_cost: Money) {
        self.base_cost = base_cost;
    }

    /// Work units can for example be
    /// number of bytes to store.
    fn work_cost(&self, num_bytes: usize) -> Money {
        // 1 nano per work unit + base cost
        Money::from_nano(num_bytes as u64 + self.base_cost.as_nano())
    }

    /// Use the factor to scale
    /// the reward per any desired formula.
    fn total_reward(&self, factor: f64, work_cost: Money) -> Money {
        let amount = factor * work_cost.as_nano() as f64;
        Money::from_nano(amount as u64)
    }

    /// Distribute the reward
    /// according to the accumulated work
    /// associated with the ids.
    fn distribute(
        &self,
        total_reward: Money,
        accounts_work: HashMap<AccountId, Work>,
    ) -> HashMap<AccountId, Money> {
        //
        let total_reward = total_reward.as_nano();
        let all_work: Work = accounts_work.values().sum();

        let mut shares_sum = 0;
        let mut shares: HashMap<AccountId, Money> = Default::default();

        for (id, work) in &accounts_work {
            let share = total_reward / (all_work / work);
            let _ = shares.insert(*id, Money::from_nano(share));
            shares_sum += share;
        }

        // covers probabilistic distribution as well
        if total_reward > shares_sum {
            if let Some(id) = random_key(&accounts_work) {
                if let Some(share) = shares.get(&id) {
                    let remainder = total_reward - shares_sum;
                    let new_share = share.as_nano() + remainder;
                    let _ = shares.insert(*id, Money::from_nano(new_share));
                }
            }
        }

        shares
            .into_iter()
            .filter(|(_, s)| s > &Money::zero())
            .collect()
    }
}

fn random_key<K: Eq + Hash, V>(map: &HashMap<K, V>) -> Option<&K> {
    if map.is_empty() {
        return None;
    }
    let index = Range::new(0, map.len()).sample(&mut rand::thread_rng());
    map.keys().skip(index).next()
}
