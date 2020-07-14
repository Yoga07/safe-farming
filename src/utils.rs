// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under the MIT license <LICENSE-MIT
// http://opensource.org/licenses/MIT> or the Modified BSD license <LICENSE-BSD
// https://opensource.org/licenses/BSD-3-Clause>, at your option. This file may not be copied,
// modified, or distributed except according to those terms. Please review the Licences for the
// specific language governing permissions and limitations relating to use of the SAFE Network
// Software.

use safe_nd::{Money, RewardCounter};

/// A util for calculating the median
/// of a vec of RewardCounters.
/// Implements Into<RewardCounter>, since
/// the semantics of this set is that it
/// basically represents a single value, which we
/// derive by taking the median of the set.
pub struct RewardCounterSet {
    counters: Vec<RewardCounter>,
}

impl RewardCounterSet {
    /// Returns the median of all values.
    pub fn median(&self) -> RewardCounter {
        let mut rewards: Vec<Money> = self
            .counters
            .clone()
            .into_iter()
            .map(|c| c.reward)
            .collect();

        rewards.sort();
        let mid = rewards.len() / 2;
        let median_reward = rewards.remove(mid);

        let mut works: Vec<u64> = self.counters.clone().into_iter().map(|c| c.work).collect();

        works.sort();
        let mid = works.len() / 2;
        let median_work = works.remove(mid);

        RewardCounter {
            reward: median_reward,
            work: median_work,
        }
    }
}

impl Into<RewardCounterSet> for Vec<RewardCounter> {
    fn into(self) -> RewardCounterSet {
        RewardCounterSet { counters: self }
    }
}

/// The semantics of RewardCounterSet is that it
/// basically represents a single value, as perceived
/// by a fix set of different actors working as a group.
/// We get the agreed value, by taking the median of the set.
/// With at least 2/3 correctly working actors (e2e), we are ensured BFT.
impl Into<RewardCounter> for RewardCounterSet {
    fn into(self) -> RewardCounter {
        self.median()
    }
}
