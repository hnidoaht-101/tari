// Copyright 2022 The Tari Project
// SPDX-License-Identifier: BSD-3-Clause

use std::{
    fmt::{Display, Formatter},
    ops::Index,
    time::Duration,
};

use chrono::{NaiveDateTime, Utc};
use multiaddr::Multiaddr;
use serde::{Deserialize, Serialize};

use crate::net_address::{multiaddr_with_stats::PeerAddressSource, MultiaddrWithStats};

/// This struct is used to store a set of different net addresses such as IPv4, IPv6, Tor or I2P for a single peer.
#[derive(Debug, Clone, Deserialize, Serialize, Default, Eq)]
pub struct MultiaddressesWithStats {
    addresses: Vec<MultiaddrWithStats>,
}

impl MultiaddressesWithStats {
    pub fn from_addresses_with_source(
        addresses: Vec<Multiaddr>,
        source: &PeerAddressSource,
    ) -> MultiaddressesWithStats {
        let mut addresses_with_stats = Vec::with_capacity(addresses.len());
        for address in addresses {
            addresses_with_stats.push(MultiaddrWithStats::new(address, source.clone()));
        }
        MultiaddressesWithStats {
            addresses: addresses_with_stats,
        }
    }

    pub fn empty() -> Self {
        MultiaddressesWithStats { addresses: Vec::new() }
    }

    /// Constructs a new list of addresses with usage stats from a list of net addresses
    pub fn new(addresses: Vec<MultiaddrWithStats>) -> MultiaddressesWithStats {
        MultiaddressesWithStats { addresses }
    }

    pub fn best(&self) -> Option<&MultiaddrWithStats> {
        self.addresses.first()
    }

    /// Provides the date and time of the last successful communication with this peer
    pub fn last_seen(&self) -> Option<NaiveDateTime> {
        let mut latest_valid_datetime: Option<NaiveDateTime> = None;
        for curr_address in &self.addresses {
            if curr_address.last_seen.is_none() {
                continue;
            }
            match latest_valid_datetime {
                Some(latest_datetime) => {
                    if latest_datetime < curr_address.last_seen.unwrap() {
                        latest_valid_datetime = curr_address.last_seen;
                    }
                },
                None => latest_valid_datetime = curr_address.last_seen,
            }
        }
        latest_valid_datetime
    }

    pub fn offline_at(&self) -> Option<NaiveDateTime> {
        let mut earliest_offline_at: Option<NaiveDateTime> = None;
        for curr_address in &self.addresses {
            // At least one address is online
            #[allow(clippy::question_mark)]
            if curr_address.offline_at().is_none() {
                return None;
            }
            match earliest_offline_at {
                Some(earliest_datetime) => {
                    if earliest_datetime > curr_address.offline_at().unwrap() {
                        earliest_offline_at = curr_address.offline_at();
                    }
                },
                None => earliest_offline_at = curr_address.offline_at(),
            }
        }
        earliest_offline_at
    }

    /// Return the time of last attempted connection to this collection of addresses
    pub fn last_attempted(&self) -> Option<NaiveDateTime> {
        let mut latest_valid_datetime: Option<NaiveDateTime> = None;
        for curr_address in &self.addresses {
            if curr_address.last_attempted.is_none() {
                continue;
            }
            match latest_valid_datetime {
                Some(latest_datetime) => {
                    if latest_datetime < curr_address.last_attempted.unwrap() {
                        latest_valid_datetime = curr_address.last_attempted;
                    }
                },
                None => latest_valid_datetime = curr_address.last_attempted,
            }
        }
        latest_valid_datetime
    }

    /// Adds a new net address to the peer. This function will not add a duplicate if the address
    /// already exists.
    pub fn add_address(&mut self, net_address: &Multiaddr, source: &PeerAddressSource) {
        if self.addresses.iter().any(|x| x.address() == net_address) {
            self.addresses
                .iter_mut()
                .find(|x| x.address() == net_address)
                .unwrap()
                .update_source_if_better(source);
        } else {
            self.addresses
                .push(MultiaddrWithStats::new(net_address.clone(), source.clone()));
            self.addresses.sort();
        }
    }

    pub fn contains(&self, net_address: &Multiaddr) -> bool {
        self.addresses.iter().any(|x| x.address() == net_address)
    }

    /// Compares the existing set of addresses to the provided address set and remove missing addresses and
    /// add new addresses without discarding the usage stats of the existing and remaining addresses.
    pub fn update_addresses(&mut self, addresses: &[Multiaddr], source: &PeerAddressSource) {
        for address in addresses {
            if let Some(addr) = self.addresses.iter_mut().find(|a| a.address() == address) {
                addr.update_source_if_better(source);
            }
        }

        let to_add = addresses
            .iter()
            .filter(|addr| !self.addresses.iter().any(|a| &a.address() == addr))
            .collect::<Vec<_>>();

        for address in to_add {
            self.addresses
                .push(MultiaddrWithStats::new(address.clone(), source.clone()));
        }

        self.addresses.sort();
    }

    /// Returns an iterator of addresses ordered from 'best' to 'worst' according to heuristics such as failed
    /// connections and latency.
    pub fn iter(&self) -> impl Iterator<Item = &Multiaddr> {
        self.addresses.iter().map(|addr| addr.address())
    }

    pub fn to_lexicographically_sorted(&self) -> Vec<Multiaddr> {
        let mut addresses = self.iter().cloned().collect::<Vec<_>>();
        addresses.sort_by(|a, b| {
            let bytes_a = a.as_ref();
            let bytes_b = b.as_ref();
            bytes_a.cmp(bytes_b)
        });
        addresses
    }

    pub fn merge(&mut self, other: &MultiaddressesWithStats) {
        for addr in &other.addresses {
            if let Some(existing) = self.find_address_mut(addr.address()) {
                existing.merge(addr);
            } else {
                self.addresses.push(addr.clone());
            }
        }
    }

    /// Finds the specified address in the set and allow updating of its variables such as its usage stats
    fn find_address_mut(&mut self, address: &Multiaddr) -> Option<&mut MultiaddrWithStats> {
        self.addresses.iter_mut().find(|a| a.address() == address)
    }

    /// The average connection latency of the provided net address will be updated to include the current measured
    /// latency sample.
    ///
    /// Returns true if the address is contained in this instance, otherwise false
    pub fn update_latency(&mut self, address: &Multiaddr, latency_measurement: Duration) -> bool {
        match self.find_address_mut(address) {
            Some(addr) => {
                addr.update_latency(latency_measurement);
                self.addresses.sort();
                true
            },
            None => false,
        }
    }

    pub fn update_address_stats<F>(&mut self, address: &Multiaddr, f: F)
    where F: FnOnce(&mut MultiaddrWithStats) {
        if let Some(addr) = self.find_address_mut(address) {
            f(addr);
            self.addresses.sort();
        }
    }

    /// Mark that a successful interaction occurred with the specified address
    ///
    /// Returns true if the address is contained in this instance, otherwise false
    pub fn mark_last_seen_now(&mut self, address: &Multiaddr) -> bool {
        match self.find_address_mut(address) {
            Some(addr) => {
                addr.mark_last_seen_now();
                addr.last_attempted = Some(Utc::now().naive_utc());
                self.addresses.sort();
                true
            },
            None => false,
        }
    }

    /// Mark that a connection could not be established with the specified net address
    ///
    /// Returns true if the address is contained in this instance, otherwise false
    pub fn mark_failed_connection_attempt(&mut self, address: &Multiaddr, failed_reason: String) -> bool {
        match self.find_address_mut(address) {
            Some(addr) => {
                addr.mark_failed_connection_attempt(failed_reason);
                addr.last_attempted = Some(Utc::now().naive_utc());
                self.addresses.sort();
                true
            },
            None => false,
        }
    }

    /// Reset the connection attempts stat on all of this Peers net addresses to retry connection
    ///
    /// Returns true if the address is contained in this instance, otherwise false
    pub fn reset_connection_attempts(&mut self) {
        for a in &mut self.addresses {
            a.reset_connection_attempts();
        }
        self.addresses.sort();
    }

    /// Returns the number of addresses
    pub fn len(&self) -> usize {
        self.addresses.len()
    }

    /// Returns if there are addresses or not
    pub fn is_empty(&self) -> bool {
        self.addresses.is_empty()
    }

    pub fn into_vec(self) -> Vec<Multiaddr> {
        self.addresses.into_iter().map(|addr| addr.address().clone()).collect()
    }

    pub fn addresses(&self) -> &[MultiaddrWithStats] {
        &self.addresses
    }
}

impl PartialEq for MultiaddressesWithStats {
    fn eq(&self, other: &Self) -> bool {
        self.addresses == other.addresses
    }
}

impl Index<usize> for MultiaddressesWithStats {
    type Output = MultiaddrWithStats;

    /// Returns the NetAddressWithStats at the given index
    fn index(&self, index: usize) -> &Self::Output {
        &self.addresses[index]
    }
}

impl From<Vec<MultiaddrWithStats>> for MultiaddressesWithStats {
    /// Constructs NetAddressesWithStats from a list of addresses with usage stats
    fn from(addresses: Vec<MultiaddrWithStats>) -> Self {
        MultiaddressesWithStats { addresses }
    }
}

impl From<MultiaddressesWithStats> for Vec<String> {
    fn from(value: MultiaddressesWithStats) -> Self {
        value
            .addresses
            .into_iter()
            .map(|addr| addr.address().to_string())
            .collect()
    }
}

impl Display for MultiaddressesWithStats {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.addresses
                .iter()
                .map(|a| a.address().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

#[cfg(test)]
mod test {
    use multiaddr::Multiaddr;

    use super::*;

    #[test]
    fn test_index_impl() {
        let net_address1 = "/ip4/123.0.0.123/tcp/8000".parse::<Multiaddr>().unwrap();
        let net_address2 = "/ip4/125.1.54.254/tcp/7999".parse::<Multiaddr>().unwrap();
        let net_address3 = "/ip4/175.6.3.145/tcp/8000".parse::<Multiaddr>().unwrap();
        let net_addresses: MultiaddressesWithStats = MultiaddressesWithStats::from_addresses_with_source(
            vec![net_address1.clone(), net_address2.clone(), net_address3.clone()],
            &PeerAddressSource::Config,
        );

        assert_eq!(net_addresses[0].address(), &net_address1);
        assert_eq!(net_addresses[1].address(), &net_address2);
        assert_eq!(net_addresses[2].address(), &net_address3);
    }

    #[test]
    fn test_last_seen() {
        let net_address1 = "/ip4/123.0.0.123/tcp/8000".parse::<Multiaddr>().unwrap();
        let net_address2 = "/ip4/125.1.54.254/tcp/7999".parse::<Multiaddr>().unwrap();
        let net_address3 = "/ip4/175.6.3.145/tcp/8000".parse::<Multiaddr>().unwrap();
        let mut net_addresses =
            MultiaddressesWithStats::from_addresses_with_source(vec![net_address1.clone()], &PeerAddressSource::Config);
        net_addresses.add_address(&net_address2, &PeerAddressSource::Config);
        net_addresses.add_address(&net_address3, &PeerAddressSource::Config);

        assert!(net_addresses.mark_last_seen_now(&net_address3));
        assert!(net_addresses.mark_last_seen_now(&net_address1));
        assert!(net_addresses.mark_last_seen_now(&net_address2));
        let desired_last_seen = net_addresses
            .addresses
            .iter()
            .max_by_key(|a| a.last_seen)
            .map(|a| a.last_seen.unwrap());
        let last_seen = net_addresses.last_seen();
        assert_eq!(desired_last_seen.unwrap(), last_seen.unwrap());
    }

    #[test]
    fn test_add_net_address() {
        let net_address1 = "/ip4/123.0.0.123/tcp/8000".parse::<Multiaddr>().unwrap();
        let net_address2 = "/ip4/125.1.54.254/tcp/7999".parse::<Multiaddr>().unwrap();
        let net_address3 = "/ip4/175.6.3.145/tcp/8000".parse::<Multiaddr>().unwrap();
        let mut net_addresses =
            MultiaddressesWithStats::from_addresses_with_source(vec![net_address1.clone()], &PeerAddressSource::Config);
        net_addresses.add_address(&net_address2, &PeerAddressSource::Config);
        net_addresses.add_address(&net_address3, &PeerAddressSource::Config);
        // Add duplicate address, test add_net_address is idempotent
        net_addresses.add_address(&net_address2, &PeerAddressSource::Config);
        assert_eq!(net_addresses.addresses.len(), 3);
        assert_eq!(net_addresses.addresses[0].address(), &net_address1);
        assert_eq!(net_addresses.addresses[1].address(), &net_address2);
        assert_eq!(net_addresses.addresses[2].address(), &net_address3);
    }

    #[test]
    fn test_get_net_address() {
        let net_address1 = "/ip4/123.0.0.123/tcp/8000".parse::<Multiaddr>().unwrap();
        let net_address2 = "/ip4/125.1.54.254/tcp/7999".parse::<Multiaddr>().unwrap();
        let net_address3 = "/ip4/175.6.3.145/tcp/8000".parse::<Multiaddr>().unwrap();
        let mut net_addresses =
            MultiaddressesWithStats::from_addresses_with_source(vec![net_address1.clone()], &PeerAddressSource::Config);
        net_addresses.add_address(&net_address2, &PeerAddressSource::Config);
        net_addresses.add_address(&net_address3, &PeerAddressSource::Config);

        let priority_address = net_addresses.iter().next().unwrap();
        assert_eq!(priority_address, &net_address1);

        net_addresses.mark_last_seen_now(&net_address1);
        net_addresses.mark_last_seen_now(&net_address2);
        net_addresses.mark_last_seen_now(&net_address3);
        assert!(net_addresses.update_latency(&net_address1, Duration::from_millis(250)));
        assert!(net_addresses.update_latency(&net_address2, Duration::from_millis(50)));
        assert!(net_addresses.update_latency(&net_address3, Duration::from_millis(100)));
        let priority_address = net_addresses.iter().next().unwrap();
        assert_eq!(priority_address, &net_address2);

        assert!(net_addresses.mark_failed_connection_attempt(&net_address2, "error".to_string()));
        let priority_address = net_addresses.iter().next().unwrap();
        assert_eq!(priority_address, &net_address3);
    }

    #[test]
    fn test_resetting_all_connection_attempts() {
        let net_address1 = "/ip4/123.0.0.123/tcp/8000".parse::<Multiaddr>().unwrap();
        let net_address2 = "/ip4/125.1.54.254/tcp/7999".parse::<Multiaddr>().unwrap();
        let net_address3 = "/ip4/175.6.3.145/tcp/8000".parse::<Multiaddr>().unwrap();
        let addresses: Vec<MultiaddrWithStats> = vec![
            MultiaddrWithStats::new(net_address1.clone(), PeerAddressSource::Config),
            MultiaddrWithStats::new(net_address2.clone(), PeerAddressSource::Config),
            MultiaddrWithStats::new(net_address3.clone(), PeerAddressSource::Config),
        ];
        let mut net_addresses = MultiaddressesWithStats::new(addresses);
        assert!(net_addresses.mark_failed_connection_attempt(&net_address1, "error".to_string()));
        assert!(net_addresses.mark_failed_connection_attempt(&net_address2, "error".to_string()));
        assert!(net_addresses.mark_failed_connection_attempt(&net_address3, "error".to_string()));
        assert!(net_addresses.mark_failed_connection_attempt(&net_address1, "error".to_string()));

        assert_eq!(net_addresses.addresses[0].connection_attempts, 1);
        assert_eq!(net_addresses.addresses[1].connection_attempts, 1);
        assert_eq!(net_addresses.addresses[2].connection_attempts, 2);
        net_addresses.reset_connection_attempts();
        assert_eq!(net_addresses.addresses[0].connection_attempts, 0);
        assert_eq!(net_addresses.addresses[1].connection_attempts, 0);
        assert_eq!(net_addresses.addresses[2].connection_attempts, 0);
    }
}
