//! Trait for performing modulo operations.
//!
//! The Modulo trait can be used for getting the modulo (instead of remainder)
//! of a number.
use rug::Integer;

pub trait Modulo<RHS = Self> {
    type Output;

    fn modulo(self, rhs: RHS) -> Self::Output;
}

pub trait OverflowingModulo<RHS = Self> {
    type Output;

    fn overflowing_modulo(self, rhs: RHS) -> (Self::Output, bool);
}

impl Modulo for i64 {
    type Output = i64;

    fn modulo(self, rhs: Self) -> Self::Output {
        ((self % rhs) + rhs) % rhs
    }
}

impl OverflowingModulo for i64 {
    type Output = i64;

    fn overflowing_modulo(self, rhs: Self) -> (Self::Output, bool) {
        if self == Self::min_value() && rhs == -1 {
            (0, true)
        } else {
            (self.modulo(rhs), false)
        }
    }
}

impl<'a> Modulo<&'a Integer> for Integer {
    type Output = Integer;

    fn modulo(self, rhs: &Self) -> Self::Output {
        ((self % rhs) + rhs) % rhs
    }
}

impl Modulo<i32> for Integer {
    type Output = Integer;

    fn modulo(self, rhs: i32) -> Self::Output {
        ((self % rhs) + rhs) % rhs
    }
}

impl Modulo for Integer {
    type Output = Integer;

    fn modulo(self, rhs: Self) -> Self::Output {
        self.modulo(&rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modulo_i64() {
        assert_eq!((-5).modulo(86_400), 86395);
    }

    #[test]
    fn test_modulo_big_integer_with_i64() {
        let a = Integer::from(-5);
        let b = 86_400;

        assert_eq!(a.modulo(b), Integer::from(86395));
    }

    #[test]
    fn test_modulo_big_integer_with_big_integer() {
        let a = Integer::from(-5);
        let b = Integer::from(86_400);

        assert_eq!(a.modulo(&b), Integer::from(86395));
    }
}
