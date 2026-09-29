/// Set of gesture kinds a recognizer may resolve (Subsurface's `OptionSet`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GestureTypes(u8);

impl GestureTypes {
    pub const NONE: Self = Self(0);
    pub const SWIPE: Self = Self(1 << 0);
    pub const MAGNIFY: Self = Self(1 << 1);
    pub const ROTATION: Self = Self(1 << 2);
    pub const ALL: Self = Self(Self::SWIPE.0 | Self::MAGNIFY.0 | Self::ROTATION.0);

    /// Builds a set from raw bits, retaining unknown bits like `OptionSet(rawValue:)`.
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    /// The raw bit representation (`rawValue` in Subsurface).
    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// `true` if every member of `other` is in `self`.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// `true` if `self` and `other` share at least one member.
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Members of `self` that are not in `other` (`subtracting` in Swift).
    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    pub const fn symmetric_difference(self, other: Self) -> Self {
        Self(self.0 ^ other.0)
    }

    pub fn insert(&mut self, other: Self) {
        *self = self.union(other);
    }

    pub fn remove(&mut self, other: Self) {
        *self = self.difference(other);
    }
}

impl std::ops::BitOr for GestureTypes {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for GestureTypes {
    fn bitor_assign(&mut self, rhs: Self) {
        self.insert(rhs);
    }
}

impl std::ops::BitAnd for GestureTypes {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        self.intersection(rhs)
    }
}

impl std::ops::BitAndAssign for GestureTypes {
    fn bitand_assign(&mut self, rhs: Self) {
        *self = self.intersection(rhs);
    }
}

impl std::ops::BitXor for GestureTypes {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self::Output {
        self.symmetric_difference(rhs)
    }
}

impl std::ops::Sub for GestureTypes {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        self.difference(rhs)
    }
}

impl std::ops::SubAssign for GestureTypes {
    fn sub_assign(&mut self, rhs: Self) {
        self.remove(rhs);
    }
}
