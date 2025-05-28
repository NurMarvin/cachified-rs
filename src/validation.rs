//! Value validation for cached entries.

use crate::{CachifiedError, Result};

/// Trait for validating cache values.
/// 
/// This trait can be implemented to provide custom validation logic for cached values.
/// If validation fails, the cached value will be considered invalid and a fresh value
/// will be retrieved.
pub trait CheckValue<T> {
    /// Validate the given value.
    /// 
    /// Returns `Ok(())` if the value is valid, or `Err(CachifiedError)` if invalid.
    fn check(&self, value: &T) -> Result<()>;
}

/// A function-based validator that can be used with closures.
pub struct FunctionValidator<F> {
    func: F,
}

impl<F> FunctionValidator<F> {
    /// Create a new function validator.
    pub fn new(func: F) -> Self {
        Self { func }
    }
}

impl<T, F> CheckValue<T> for FunctionValidator<F>
where
    F: Fn(&T) -> Result<()>,
{
    fn check(&self, value: &T) -> Result<()> {
        (self.func)(value)
    }
}

/// A validator that always passes (no validation).
pub struct NoValidator;

impl<T> CheckValue<T> for NoValidator {
    fn check(&self, _value: &T) -> Result<()> {
        Ok(())
    }
}

/// A validator that checks if a value is not None (for Option types).
pub struct NonNullValidator;

impl<T> CheckValue<Option<T>> for NonNullValidator {
    fn check(&self, value: &Option<T>) -> Result<()> {
        match value {
            Some(_) => Ok(()),
            None => Err(CachifiedError::validation("Value is None")),
        }
    }
}

/// A validator that checks if a string is not empty.
pub struct NonEmptyStringValidator;

impl CheckValue<String> for NonEmptyStringValidator {
    fn check(&self, value: &String) -> Result<()> {
        if value.is_empty() {
            Err(CachifiedError::validation("String is empty"))
        } else {
            Ok(())
        }
    }
}

impl CheckValue<&str> for NonEmptyStringValidator {
    fn check(&self, value: &&str) -> Result<()> {
        if value.is_empty() {
            Err(CachifiedError::validation("String is empty"))
        } else {
            Ok(())
        }
    }
}

/// Helper function to create a function validator from a closure.
/// 
/// # Examples
/// 
/// ```rust
/// use cachified::validation::validator;
/// 
/// let positive_number_validator = validator(|x: &i32| {
///     if *x > 0 {
///         Ok(())
///     } else {
///         Err("Number must be positive".into())
///     }
/// });
/// ```
pub fn validator<T, F>(func: F) -> FunctionValidator<F>
where
    F: Fn(&T) -> Result<()>,
{
    FunctionValidator::new(func)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_validator() {
        let validator = validator(|x: &i32| {
            if *x > 0 {
                Ok(())
            } else {
                Err(CachifiedError::validation("Must be positive"))
            }
        });

        assert!(validator.check(&5).is_ok());
        assert!(validator.check(&-1).is_err());
    }

    #[test]
    fn test_non_null_validator() {
        let validator = NonNullValidator;
        
        assert!(validator.check(&Some(42)).is_ok());
        assert!(validator.check(&None::<i32>).is_err());
    }

    #[test]
    fn test_non_empty_string_validator() {
        let validator = NonEmptyStringValidator;
        
        assert!(validator.check(&"hello".to_string()).is_ok());
        assert!(validator.check(&"".to_string()).is_err());
        assert!(validator.check(&"world").is_ok());
        assert!(validator.check(&"").is_err());
    }

    #[test]
    fn test_no_validator() {
        let validator = NoValidator;
        
        assert!(validator.check(&42).is_ok());
        assert!(validator.check(&"anything").is_ok());
        assert!(validator.check(&None::<String>).is_ok());
    }
}
