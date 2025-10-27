use std::any::Any;
use std::panic;
use crate::core::application::messages::{ProviderMessage};
use crate::Image;

pub trait ProviderTrait {
    fn get_functionalities() -> ProviderMessage;
    fn execute(&self, method: &str, input: Box<dyn Any>) -> Box<dyn Any>;
}