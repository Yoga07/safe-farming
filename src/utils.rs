// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under the MIT license <LICENSE-MIT
// http://opensource.org/licenses/MIT> or the Modified BSD license <LICENSE-BSD
// https://opensource.org/licenses/BSD-3-Clause>, at your option. This file may not be copied,
// modified, or distributed except according to those terms. Please review the Licences for the
// specific language governing permissions and limitations relating to use of the SAFE Network
// Software.

use safe_nd::{Error, Money, Result, RewardCounter};

/// A util for calculating the median
/// of a vec of RewardCounters.
/// Implements Into<RewardCounter>, since
/// the semantics of this set is that it
/// basically represents a single value, which we
/// derive by taking the median of the set.
pub struct RewardCounterSet {
    quorum: usize,
    counters: Vec<RewardCounter>,
}

/// The semantics of RewardCounterSet is that it
/// basically represents a single value, as perceived
/// by a fix set of different actors working as a group.
/// We get the agreed value, by taking the median of the set.
/// With at least 2/3 correctly working actors (e2e), we are ensured BFT.
impl RewardCounterSet {
    /// The number of expected counters determines
    /// when we have an agreed value. Must be uneven.
    /// The vec can be empty or contain any number already.
    pub fn new(expected_counters: usize, counters: Vec<RewardCounter>) -> Result<Self> {
        if expected_counters % 2 == 0 || 3 > expected_counters {
            return Err(Error::InvalidOperation);
        }
        let quorum = (expected_counters / 3) * 2;
        Ok(Self { quorum, counters })
    }

    /// Returns the length of the set.
    pub fn len(&self) -> usize {
        self.counters.len()
    }

    /// Returns whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.counters.is_empty()
    }

    /// Adds a counter to the set.
    pub fn add(&mut self, counter: RewardCounter) {
        self.counters.push(counter)
    }

    /// Returns the agreed value between all,
    /// interpreted through the median value.
    pub fn agreed_value(&self) -> Option<RewardCounter> {
        let count = self.counters.len();
        if self.quorum > count {
            return None;
        }

        let median_reward = self.median_reward();
        let median_work = self.median_work();

        Some(RewardCounter {
            reward: median_reward,
            work: median_work,
        })
    }

    fn median_reward(&self) -> Money {
        let mut rewards: Vec<Money> = self
            .counters
            .clone()
            .into_iter()
            .map(|c| c.reward)
            .collect();

        rewards.sort();

        if rewards.len() % 2 == 0 {
            let mid_0 = rewards.len() / 2;
            let mid_1 = (rewards.len() / 2) + 1;
            let mid_0 = rewards.clone().remove(mid_0).as_nano();
            let mid_1 = rewards.remove(mid_1).as_nano();
            Money::from_nano((mid_0 + mid_1) / 2)
        } else {
            let mid = rewards.len() / 2;
            rewards.remove(mid)
        }
    }

    fn median_work(&self) -> u64 {
        let mut works: Vec<u64> = self.counters.clone().into_iter().map(|c| c.work).collect();
        works.sort();

        if works.len() % 2 == 0 {
            let mid_0 = works.len() / 2;
            let mid_1 = (works.len() / 2) + 1;
            let mid_0 = works.clone().remove(mid_0);
            let mid_1 = works.remove(mid_1);
            (mid_0 + mid_1) / 2
        } else {
            let mid = works.len() / 2;
            works.remove(mid)
        }
    }
}
