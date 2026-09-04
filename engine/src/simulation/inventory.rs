pub const DEFAULT_CARRYING_CAPACITY: u16 = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum ItemKind {
    Food,
    Timber,
    Stone,
    Iron,
}

impl ItemKind {
    pub const ALL: [Self; 4] = [Self::Food, Self::Timber, Self::Stone, Self::Iron];
    const fn index(self) -> usize {
        self as usize
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Food => "Food",
            Self::Timber => "Timber",
            Self::Stone => "Stone",
            Self::Iron => "Iron",
        }
    }

    pub fn from_slug(value: &str) -> Option<Self> {
        match value {
            "food" => Some(Self::Food),
            "timber" => Some(Self::Timber),
            "stone" => Some(Self::Stone),
            "iron" => Some(Self::Iron),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inventory {
    capacity: u16,
    amounts: [u16; ItemKind::ALL.len()],
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new(DEFAULT_CARRYING_CAPACITY)
    }
}

impl Inventory {
    pub const fn new(capacity: u16) -> Self {
        Self {
            capacity,
            amounts: [0; ItemKind::ALL.len()],
        }
    }
    pub const fn capacity(&self) -> u16 {
        self.capacity
    }
    pub const fn amounts(&self) -> &[u16; ItemKind::ALL.len()] {
        &self.amounts
    }
    pub fn used_capacity(&self) -> u16 {
        self.amounts.iter().copied().sum()
    }
    pub fn remaining_capacity(&self) -> u16 {
        self.capacity.saturating_sub(self.used_capacity())
    }
    pub const fn amount(&self, kind: ItemKind) -> u16 {
        self.amounts[kind.index()]
    }
    pub fn add(&mut self, kind: ItemKind, quantity: u16) -> u16 {
        let accepted = quantity.min(self.remaining_capacity());
        self.amounts[kind.index()] += accepted;
        accepted
    }
    pub fn remove(&mut self, kind: ItemKind, quantity: u16) -> u16 {
        let slot = &mut self.amounts[kind.index()];
        let removed = quantity.min(*slot);
        *slot -= removed;
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn additions_share_one_bounded_capacity() {
        let mut inventory = Inventory::new(10);
        assert_eq!(inventory.add(ItemKind::Food, 7), 7);
        assert_eq!(inventory.add(ItemKind::Timber, 8), 3);
        assert_eq!(inventory.add(ItemKind::Stone, 1), 0);
        assert_eq!(
            (inventory.used_capacity(), inventory.remaining_capacity()),
            (10, 0)
        );
    }
    #[test]
    fn removal_never_underflows_and_releases_capacity() {
        let mut inventory = Inventory::new(5);
        inventory.add(ItemKind::Iron, 4);
        assert_eq!(inventory.remove(ItemKind::Iron, 10), 4);
        assert_eq!(inventory.remove(ItemKind::Iron, 1), 0);
        assert_eq!(
            (inventory.used_capacity(), inventory.remaining_capacity()),
            (0, 5)
        );
    }
    #[test]
    fn identical_operation_sequences_are_deterministic() {
        let run = || {
            let mut value = Inventory::new(12);
            value.add(ItemKind::Stone, 5);
            value.add(ItemKind::Food, 9);
            value.remove(ItemKind::Stone, 2);
            value
        };
        assert_eq!(run(), run());
    }
}
