//! Fixed, allocation-free bridge errors.
use tiforth_memory::MemoryError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BridgeError {
    InvalidBudget,
    PoolMismatch,
    InconsistentBacking,
    AlreadyInitialized,
    Uninitialized,
    Closed,
    Contended,
    StaleEpoch,
    EpochExhausted,
    SlotsExhausted,
    PendingOwners,
    InvalidLayout,
    Memory(MemoryError),
}

impl From<MemoryError> for BridgeError {
    fn from(value: MemoryError) -> Self {
        Self::Memory(value)
    }
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for BridgeError {}
