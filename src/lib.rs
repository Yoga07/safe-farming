// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

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

use safe_nd::{AccountId, Money};
use std::collections::HashMap;
// pub use crate::{
//     farming::xx, farming::xx,
// };

///
mod accumulation;
mod calculation;
mod example;

pub use self::accumulation::Accumulation;

type WorkCounter = u64;

///
#[derive(Clone, Eq, PartialEq, PartialOrd, Debug)]
pub struct CurrentAccumulation {
    ///
    pub amount: Money,
    ///
    pub worked: WorkCounter,
}

impl CurrentAccumulation {
    ///
    pub fn add(&self, amount: Money) -> Option<Self> {
        let sum = match self.amount.checked_add(amount) {
            Some(s) => s,
            None => return None,
        };
        Some(Self {
            worked: self.worked + 1,
            amount: sum,
        })
    }
}

impl Default for CurrentAccumulation {
    fn default() -> Self {
        Self {
            worked: 0,
            amount: Money::zero(),
        }
    }
}

///
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum AccumulationEvent {
    ///
    AccountAdded(AccountAdded),
    ///
    AmountsAccumulated(AmountsAccumulated),
    ///
    AccumulatedClaimed(AccumulatedClaimed),
}

///
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct AccountAdded {
    ///
    pub id: AccountId,
    ///
    pub worked: WorkCounter,
}

///
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct AmountsAccumulated {
    /// An identifier of a rewarded "thing", such as a data hash for example.
    /// Makes sure we only accumulate a rewarded action _once_.
    pub id: Vec<u8>,
    ///
    pub distribution: HashMap<AccountId, Money>,
}

///
#[derive(Clone, Eq, PartialEq, PartialOrd, Debug)]
pub struct AccumulatedClaimed {
    ///
    pub account: AccountId,
    ///
    pub accumulated: CurrentAccumulation,
}