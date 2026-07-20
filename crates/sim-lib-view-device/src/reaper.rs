//! Deterministic retention reaper for device samples and content refs.

use std::collections::BTreeMap;

use sim_kernel::{Expr, Symbol};

use crate::{ConsentReceipt, FrameClock};

/// Stable key for a stored device sample or referenced content value.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StoreKey(Symbol);

impl StoreKey {
    /// Builds a store key from a symbol.
    pub fn new(symbol: Symbol) -> Self {
        Self(symbol)
    }

    /// Builds a store key in the `device/store` namespace.
    pub fn named(name: impl Into<String>) -> Self {
        Self(Symbol::qualified("device/store", name.into()))
    }

    /// Returns the backing symbol.
    pub fn as_symbol(&self) -> &Symbol {
        &self.0
    }
}

/// One stored sample governed by a consent receipt.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredSample {
    key: StoreKey,
    receipt_seq: u64,
    tick: u64,
    content_refs: Vec<StoreKey>,
    value: Expr,
}

impl StoredSample {
    /// Builds a stored sample.
    pub fn new(
        key: StoreKey,
        receipt_seq: u64,
        tick: u64,
        content_refs: Vec<StoreKey>,
        value: Expr,
    ) -> Self {
        Self {
            key,
            receipt_seq,
            tick,
            content_refs,
            value,
        }
    }

    /// Returns this sample's key.
    pub fn key(&self) -> &StoreKey {
        &self.key
    }

    /// Returns the receipt sequence governing this sample.
    pub fn receipt_seq(&self) -> u64 {
        self.receipt_seq
    }

    /// Returns the modeled insertion tick.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Returns the referenced content keys.
    pub fn content_refs(&self) -> &[StoreKey] {
        &self.content_refs
    }

    /// Returns the stored expression.
    pub fn value(&self) -> &Expr {
        &self.value
    }
}

/// In-memory sample/content store used by deterministic device tests.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviceSampleStore {
    samples: BTreeMap<StoreKey, StoredSample>,
    content: BTreeMap<StoreKey, Expr>,
}

impl DeviceSampleStore {
    /// Builds an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts referenced content.
    pub fn insert_content(&mut self, key: StoreKey, value: Expr) {
        self.content.insert(key, value);
    }

    /// Inserts a stored sample.
    pub fn insert_sample(&mut self, sample: StoredSample) {
        self.samples.insert(sample.key.clone(), sample);
    }

    /// Returns true when a sample is present.
    pub fn contains_sample(&self, key: &StoreKey) -> bool {
        self.samples.contains_key(key)
    }

    /// Returns true when referenced content is present.
    pub fn contains_content(&self, key: &StoreKey) -> bool {
        self.content.contains_key(key)
    }

    /// Returns the number of stored samples.
    pub fn sample_len(&self) -> usize {
        self.samples.len()
    }

    /// Returns a stored sample by key.
    pub fn sample(&self, key: &StoreKey) -> Option<&StoredSample> {
        self.samples.get(key)
    }

    /// Returns the number of referenced content entries.
    pub fn content_len(&self) -> usize {
        self.content.len()
    }
}

/// Eviction record produced by a retention sweep.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evicted {
    /// Key that was removed.
    pub key: StoreKey,
    /// Reason for eviction.
    pub reason: Symbol,
}

/// Retention/redaction action selected for a stored device value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyMode {
    /// Keep the value while the retention window remains open.
    Retain,
    /// Redact the value under the reaper contract.
    Redact,
    /// Delete the value under the reaper contract.
    Delete,
}

/// Reaper directive derived from consent receipt policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReaperDirective {
    /// Reaper action.
    pub mode: PrivacyMode,
    /// Symbols naming fields or streams to redact.
    pub redact: Vec<Symbol>,
}

impl ReaperDirective {
    /// Builds a directive from a receipt.
    pub fn from_receipt(receipt: &ConsentReceipt) -> Self {
        let mode = if receipt.redact.is_empty() {
            PrivacyMode::Retain
        } else {
            PrivacyMode::Redact
        };
        Self {
            mode,
            redact: receipt.redact.clone(),
        }
    }
}

/// Deterministic modeled-clock retention reaper.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RetentionReaper;

impl RetentionReaper {
    /// Builds a retention reaper.
    pub fn new() -> Self {
        Self
    }

    /// Sweeps samples and referenced content older than their consent window.
    pub fn sweep(
        &self,
        store: &mut DeviceSampleStore,
        receipts: &[ConsentReceipt],
        now: FrameClock,
    ) -> Vec<Evicted> {
        let receipt_by_seq: BTreeMap<u64, &ConsentReceipt> = receipts
            .iter()
            .map(|receipt| (receipt.seq, receipt))
            .collect();
        for sample in store.samples.values_mut() {
            if let Some(receipt) = receipt_by_seq.get(&sample.receipt_seq)
                && now.elapsed_ms_since(sample.tick) <= receipt.retain_ms
            {
                apply_redaction(&mut sample.value, &receipt.redact);
            }
        }
        let expired: Vec<StoreKey> = store
            .samples
            .values()
            .filter(|sample| {
                receipt_by_seq
                    .get(&sample.receipt_seq)
                    .is_none_or(|receipt| now.elapsed_ms_since(sample.tick) > receipt.retain_ms)
            })
            .map(|sample| sample.key.clone())
            .collect();

        let mut evicted = Vec::new();
        for key in expired {
            if let Some(sample) = store.samples.remove(&key) {
                evicted.push(Evicted {
                    key,
                    reason: retention_reason(),
                });
                for content_key in sample.content_refs {
                    if store.content.remove(&content_key).is_some() {
                        evicted.push(Evicted {
                            key: content_key,
                            reason: retention_reason(),
                        });
                    }
                }
            }
        }
        evicted
    }
}

/// Returns the stable retention-eviction reason.
pub fn retention_reason() -> Symbol {
    Symbol::qualified("device/reaper", "retention")
}

fn redacted_marker() -> Expr {
    Expr::Symbol(Symbol::qualified("device/reaper", "redacted"))
}

fn apply_redaction(value: &mut Expr, redact: &[Symbol]) {
    if redact.is_empty() {
        return;
    }
    let Expr::Map(entries) = value else {
        return;
    };
    for (key, field_value) in entries {
        if let Expr::Symbol(field) = key
            && redact
                .iter()
                .any(|directive| redacts_field(directive, field))
        {
            *field_value = redacted_marker();
        }
    }
}

fn redacts_field(directive: &Symbol, field: &Symbol) -> bool {
    directive == field
        || directive.name == field.name
        || directive.as_qualified_str() == field.as_qualified_str()
}
