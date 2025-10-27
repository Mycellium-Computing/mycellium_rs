use std::any::Any;
use crate::core::application::messages::{ProviderMessage};

pub trait ProviderTrait {
    fn get_functionalities(&self) -> ProviderMessage;
    fn execute(&self, method: &str, input: Box<dyn Any>) -> Box<dyn Any>;
}