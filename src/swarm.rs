//! How far an agent may go in creating agents of its own.
//!
//! A peer runs headless, which means it approves its own tool calls, and it is
//! launched with its parent's configuration — so a `spawn`-enabled agent
//! produces `spawn`-enabled children. Without a bound, one instruction can grow
//! a tree of processes that nobody asked for and nothing stops.
//!
//! The bound is this module: a pure decision over (live children, depth,
//! limits), so the whole policy is one table and one unit test. It governs the
//! **`spawn` tool** — the autonomous path. `agent-cli spawn` and `/spawn` are a
//! person deciding, and are deliberately not bounded.

use crate::config::SpawnConfig;

/// What `[spawn]` allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnLimits {
    /// Live direct children one agent may have. 0 forbids autonomous spawning.
    pub max_children: u32,
    /// How deep a chain of spawned agents may go. An agent at this depth may
    /// not spawn, so 0 also forbids it.
    pub max_depth: u32,
}

impl Default for SpawnLimits {
    /// The shipped defaults, so a test or a caller without configuration gets
    /// the same policy a fresh install has.
    fn default() -> Self {
        Self::from(&SpawnConfig::default())
    }
}

impl From<&SpawnConfig> for SpawnLimits {
    fn from(cfg: &SpawnConfig) -> Self {
        Self {
            max_children: cfg.max_children,
            max_depth: cfg.max_depth,
        }
    }
}

/// The verdict on one `spawn` tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Allowance {
    Allowed,
    /// Refused, carrying the sentence the model is given: which limit, its
    /// configured value, and what the agent would have to change.
    Denied(String),
}

/// May an agent at `depth`, with `live_children` live direct children, create
/// one more?
///
/// Depth is answered before breadth: being too deep is the more fundamental
/// refusal — no amount of stopping children changes it — so reporting it first
/// tells the model something it can act on.
pub fn spawn_allowance(live_children: u32, depth: u32, limits: SpawnLimits) -> Allowance {
    if limits.max_children == 0 {
        return Allowance::Denied(
            "spawning is disabled: [spawn] max_children is 0. No agent may create another."
                .to_string(),
        );
    }
    if depth >= limits.max_depth {
        return Allowance::Denied(format!(
            "spawn depth limit reached: this agent is at depth {depth} and [spawn] max_depth is \
             {}. Agents this deep may not create more agents; do the work here, or ask the agent \
             that created you.",
            limits.max_depth
        ));
    }
    if live_children >= limits.max_children {
        return Allowance::Denied(format!(
            "child limit reached: {live_children} live child agent(s) and [spawn] max_children is \
             {}. Stop one with stop_agent before creating another, or reuse an existing child \
             with send_to.",
            limits.max_children
        ));
    }
    Allowance::Allowed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(max_children: u32, max_depth: u32) -> SpawnLimits {
        SpawnLimits {
            max_children,
            max_depth,
        }
    }

    #[test]
    fn under_both_limits_is_allowed() {
        assert_eq!(spawn_allowance(0, 0, limits(4, 2)), Allowance::Allowed);
        assert_eq!(spawn_allowance(3, 1, limits(4, 2)), Allowance::Allowed);
    }

    #[test]
    fn the_child_limit_binds_at_the_limit_not_past_it() {
        // 4 live children against a limit of 4: the next one would be the 5th.
        let denied = spawn_allowance(4, 0, limits(4, 2));
        match denied {
            Allowance::Denied(why) => {
                assert!(why.contains("child limit reached"), "{why}");
                assert!(why.contains('4'), "the limit and the count are named: {why}");
                assert!(why.contains("stop_agent"), "it says what would help: {why}");
            }
            Allowance::Allowed => panic!("4 children against a limit of 4 must be denied"),
        }
        // Past the limit denies too, reporting the real count.
        match spawn_allowance(5, 0, limits(4, 2)) {
            Allowance::Denied(why) => assert!(why.contains("5 live child agent(s)"), "{why}"),
            Allowance::Allowed => panic!("past the limit must be denied"),
        }
    }

    #[test]
    fn the_depth_limit_stops_an_agent_that_is_deep_enough() {
        assert_eq!(spawn_allowance(0, 1, limits(4, 2)), Allowance::Allowed);
        match spawn_allowance(0, 2, limits(4, 2)) {
            Allowance::Denied(why) => {
                assert!(why.contains("depth limit reached"), "{why}");
                assert!(why.contains("depth 2"), "the agent's own depth is named: {why}");
            }
            Allowance::Allowed => panic!("an agent at the depth limit must be denied"),
        }
    }

    #[test]
    fn depth_is_reported_when_both_limits_are_exceeded() {
        // Stopping children would not help here, so depth is the useful answer.
        match spawn_allowance(9, 9, limits(4, 2)) {
            Allowance::Denied(why) => assert!(why.contains("depth limit reached"), "{why}"),
            Allowance::Allowed => panic!("must be denied"),
        }
    }

    #[test]
    fn zero_switches_autonomous_spawning_off() {
        match spawn_allowance(0, 0, limits(0, 5)) {
            Allowance::Denied(why) => assert!(why.contains("spawning is disabled"), "{why}"),
            Allowance::Allowed => panic!("max_children = 0 must deny"),
        }
        // max_depth = 0 denies too: the root itself is already at the limit.
        match spawn_allowance(0, 0, limits(4, 0)) {
            Allowance::Denied(why) => assert!(why.contains("depth limit reached"), "{why}"),
            Allowance::Allowed => panic!("max_depth = 0 must deny"),
        }
    }

    #[test]
    fn the_limits_come_from_the_config_section() {
        let cfg = SpawnConfig {
            max_children: 2,
            max_depth: 1,
        };
        let limits = SpawnLimits::from(&cfg);
        assert_eq!(limits.max_children, 2);
        assert_eq!(limits.max_depth, 1);
        assert_eq!(spawn_allowance(1, 0, limits), Allowance::Allowed);
        assert!(matches!(spawn_allowance(2, 0, limits), Allowance::Denied(_)));
        assert!(matches!(spawn_allowance(0, 1, limits), Allowance::Denied(_)));
    }
}
