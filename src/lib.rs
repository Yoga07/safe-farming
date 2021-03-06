// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under the MIT license <LICENSE-MIT
// http://opensource.org/licenses/MIT> or the Modified BSD license <LICENSE-BSD
// https://opensource.org/licenses/BSD-3-Clause>, at your option. This file may not be copied,
// modified, or distributed except according to those terms. Please review the Licences for the
// specific language governing permissions and limitations relating to use of the SAFE Network
// Software.

//! Implementation of Farming Rewards for the SAFE Network.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/maidsafe/QA/master/Images/maidsafe_logo.png",
    html_favicon_url = "https://maidsafe.net/img/favicon.ico",
    test(attr(forbid(warnings)))
)]
// For explanation of lint checks, run `rustc -W help`.
#![forbid(unsafe_code)]
#![warn(
    // TODO: add missing debug implementations for structs?
    // missing_debug_implementations,
    missing_docs,
    trivial_casts,
    trivial_numeric_casts,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications,
    unused_results
)]
// For quick_error
#![recursion_limit = "128"]

pub use crate::{
    accumulation::Accumulation,
    calculation::{RewardAlgo, StorageRewards},
    utils::RewardCounterSet,
};
use safe_nd::{AccountId, Money, RewardCounter, Work};
use std::collections::HashMap;

///
pub mod accumulation;
///
pub mod calculation;
/// Used for calculating the median
/// of a vec of RewardCounters.
pub mod utils;

mod example;

///
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum AccumulationEvent {
    ///
    AccountAdded(AccountAdded),
    ///
    RewardsAccumulated(RewardsAccumulated),
    ///
    RewardsClaimed(RewardsClaimed),
}

///
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct AccountAdded {
    /// The account id.
    pub id: AccountId,
    /// Total work accumulated by the account owner.
    pub work: Work,
}

/// Reward and its distribution has been
/// calculated, and accumulates with this event.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct RewardsAccumulated {
    /// An identifier of a rewarded "thing", such as a data hash for example.
    /// Makes sure we only accumulate a rewarded action _once_.
    pub id: Vec<u8>,
    ///
    pub distribution: HashMap<AccountId, Money>,
}

/// The accumulation of rewards stops at
/// this instance of the Accumulator.
/// The accumulated work is transfered to another instance,
/// and the accumulated rewards is paid out.
#[derive(Clone, Eq, PartialEq, PartialOrd, Debug)]
pub struct RewardsClaimed {
    ///
    pub account: AccountId,
    ///
    pub rewards: RewardCounter,
}

#[cfg(test)]
mod test {
    use super::{Accumulation, AccumulationEvent};
    use safe_nd::{Error, Money, PublicKey, Result};
    use threshold_crypto::SecretKey;

    macro_rules! hashmap {
        ($( $key: expr => $val: expr ),*) => {{
             let mut map = ::std::collections::HashMap::new();
             $( let _ = map.insert($key, $val); )*
             map
        }}
    }

    #[test]
    fn when_data_was_not_previously_rewarded_reward_accumulates() -> Result<()> {
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
        match acc.get(&account) {
            None => return Err(Error::NoSuchKey),
            Some(accumulated) => assert_eq!(accumulated.reward, reward),
        };
        Ok(())
    }

    fn get_random_pk() -> PublicKey {
        PublicKey::from(SecretKey::random().public_key())
    }
}
