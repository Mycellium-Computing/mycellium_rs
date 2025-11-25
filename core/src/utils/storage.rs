use core::any::Any;

pub struct ExecutionObjects {
    objects: Vec<Box<dyn Any + Send>>,
}

impl ExecutionObjects {
    pub fn new() -> Self {
        ExecutionObjects {
            objects: Vec::new(),
        }
    }

    pub fn save<T: Any + Send>(&mut self, object: T) {
        self.objects.push(Box::new(object));
    }
}
