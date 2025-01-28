pub const THREE_MONTHS_SECONDS: i64 = 60 * 60 * 24 * 90;
pub const MAX_MILESTONES: usize = 8;
pub const MAX_USERS: usize = 1000;
pub const MAX_HOLD_AMOUNT: u64 = 50000000; // Anti-whale restriction:
pub const MAX_EXEMPTED_WALLETS: usize = 20; // Maximum exempted wallets
// Constants for Milestone size
pub const MILESTONE_SIZE: usize = 8 + 1; // u64 (8 bytes) + u8 (1 byte)