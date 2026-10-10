//! Public numeric usage only. Never treat a cumulative thread total as a turn's bill.
use crate::model::EventPayload;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Totals([Option<u64>; 5]);
impl Totals {
    pub(super) fn read(value: &Value) -> Option<Self> {
        let mut counts = [None; 5];
        for (index, key) in [
            "inputTokens",
            "outputTokens",
            "cachedInputTokens",
            "cacheWriteInputTokens",
            "reasoningOutputTokens",
        ]
        .iter()
        .enumerate()
        {
            if let Some(value) = value.get(key) {
                counts[index] = Some(value.as_u64().filter(|n| *n <= 1_000_000_000_000)?);
            }
        }
        counts.iter().any(Option::is_some).then_some(Self(counts))
    }
    fn delta(self, previous: Self) -> Option<Self> {
        let mut counts = [None; 5];
        for (index, (now, old)) in self.0.iter().zip(previous.0).enumerate() {
            counts[index] = match (now, old) {
                (Some(now), Some(old)) => Some(now.checked_sub(old)?),
                _ => None,
            };
        }
        Some(Self(counts))
    }
}

#[derive(Debug)]
pub(super) struct Tracker {
    previous: Option<Totals>,
    latest: Option<(Totals, Option<Totals>)>,
    sum: [Option<u64>; 5],
    incomplete: bool,
    observed: bool,
}
impl Default for Tracker {
    fn default() -> Self {
        Self::new(false, None)
    }
}
impl Tracker {
    pub(super) fn new(fresh: bool, baseline: Option<Totals>) -> Self {
        Self {
            previous: if fresh {
                Some(Totals([Some(0); 5]))
            } else {
                baseline
            },
            latest: None,
            sum: [None; 5],
            incomplete: false,
            observed: false,
        }
    }
    pub(super) fn restore(&mut self, value: &Value) {
        if !self.observed && self.previous.is_none() {
            self.previous = Totals::read(&value["total"]);
        }
    }
    pub(super) fn observe(&mut self, value: &Value) {
        let Some(total) = Totals::read(&value["total"]) else {
            self.incomplete = true;
            return;
        };
        let last = Totals::read(&value["last"]);
        if [0, 1, 2, 4].iter().any(|index| total.0[*index].is_none())
            || self
                .previous
                .is_some_and(|old| [0, 1, 2, 4].iter().any(|index| old.0[*index].is_none()))
        {
            self.incomplete = true;
        }
        if self.latest == Some((total, last)) {
            return;
        }
        // Same cumulative total means no additional observed usage, even if `last` changed.
        if self.latest.is_some_and(|(old, _)| old == total) {
            return;
        }
        let delta = self
            .previous
            .and_then(|old| total.delta(old))
            .unwrap_or_else(|| {
                self.incomplete = true;
                last.unwrap_or(Totals([None; 5]))
            });
        for (sum, increment) in self.sum.iter_mut().zip(delta.0) {
            if let Some(value) = increment {
                let next = sum.unwrap_or(0).saturating_add(value);
                if next > 1_000_000_000_000 {
                    self.incomplete = true;
                }
                *sum = Some(next.min(1_000_000_000_000));
            }
        }
        self.previous = Some(total);
        self.latest = Some((total, last));
        self.observed = true;
    }
    pub(super) fn take(&mut self) -> Option<EventPayload> {
        if !self.observed {
            return None;
        }
        self.observed = false;
        Some(EventPayload::ProviderUsage {
            provider: "openai".into(),
            model_requests: None,
            input_tokens: self.sum[0],
            output_tokens: self.sum[1],
            cache_read_tokens: self.sum[2],
            cache_creation_tokens: self.sum[3],
            reasoning_tokens: self.sum[4],
            incomplete: self.incomplete,
        })
    }
    pub(super) fn mark_incomplete(&mut self) {
        self.incomplete = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn usage(input: u64, output: u64) -> Value {
        json!({"inputTokens":input,"outputTokens":output,"cachedInputTokens":input/2,"reasoningOutputTokens":output/2})
    }
    #[test]
    fn resumed_totals_are_differenced_and_replayed_notifications_are_not_counted_twice() {
        let baseline = Totals::read(&usage(100, 20));
        let mut tracker = Tracker::new(false, baseline);
        let first = json!({"total":usage(130,30),"last":usage(30,10)});
        tracker.observe(&first);
        tracker.observe(&first);
        tracker.observe(&json!({"total":usage(130,30),"last":usage(99,99)}));
        tracker.observe(&json!({"total":usage(170,50),"last":usage(40,20)}));
        assert!(matches!(
            tracker.take(),
            Some(EventPayload::ProviderUsage {
                input_tokens: Some(70),
                output_tokens: Some(30),
                cache_read_tokens: Some(35),
                reasoning_tokens: Some(15),
                incomplete: false,
                ..
            })
        ));
        assert!(tracker.take().is_none());
    }
    #[test]
    fn fresh_threads_restore_unknown_baselines_and_counter_resets_are_explicit() {
        let mut fresh = Tracker::new(true, None);
        fresh.observe(&json!({"total":usage(10,2),"last":usage(10,2)}));
        assert!(matches!(
            fresh.take(),
            Some(EventPayload::ProviderUsage {
                input_tokens: Some(10),
                incomplete: false,
                ..
            })
        ));
        let mut resumed = Tracker::new(false, None);
        resumed.restore(&json!({"total":usage(100,20)}));
        resumed.observe(&json!({"total":usage(110,22),"last":usage(10,2)}));
        assert!(matches!(
            resumed.take(),
            Some(EventPayload::ProviderUsage {
                input_tokens: Some(10),
                incomplete: false,
                ..
            })
        ));
        let mut missing = Tracker::new(false, None);
        missing.observe(&json!({"total":usage(110,22),"last":usage(10,2)}));
        missing.observe(&json!({"total":usage(5,1),"last":usage(5,1)}));
        assert!(matches!(
            missing.take(),
            Some(EventPayload::ProviderUsage {
                input_tokens: Some(15),
                incomplete: true,
                ..
            })
        ));
        assert!(Totals::read(&json!({"inputTokens":-1})).is_none());
        assert!(Totals::read(&json!({"inputTokens":"secret"})).is_none());
    }
    #[test]
    fn missing_required_fields_and_malformed_followups_cannot_be_claimed_complete() {
        let mut partial = Tracker::new(true, None);
        partial.observe(&json!({"total":{"inputTokens":10}}));
        partial.observe(&json!({"total":{"inputTokens":"invalid"}}));
        assert!(matches!(
            partial.take(),
            Some(EventPayload::ProviderUsage {
                input_tokens: Some(10),
                output_tokens: None,
                incomplete: true,
                ..
            })
        ));
    }
}
