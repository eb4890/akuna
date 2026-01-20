use anyhow::Result;
use wasmtime::component::Val;
use std::sync::Arc;

pub struct CallContext {
    pub target_component: String,
    pub target_interface: String,
    pub function_name: String,
    pub caller_component: Option<String>,
}

// Next middleware in the chain
pub type Next = Box<dyn Fn(Vec<Val>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<Val>>> + Send>> + Send + Sync>;

pub trait Middleware: Send + Sync {
    fn handle(
        &self,
        ctx: &CallContext,
        params: Vec<Val>,
        next: Next,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<Val>>> + Send>>;

    fn as_any(&self) -> &dyn std::any::Any;
}

// Implementations

pub struct LoggingMiddleware;

impl Middleware for LoggingMiddleware {
    fn handle(
        &self,
        ctx: &CallContext,
        params: Vec<Val>,
        next: Next,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<Val>>> + Send>> {
        let func_name = ctx.function_name.clone();
        let target = ctx.target_component.clone();
        
        // Clone params for logging (Val is Clone-ish, actually Val is cheap clone? No, Val can contain resources.)
        // formatting Val is hard if it consumes resources. 
        // We can just print "Args count" or try debug print if supported.
        // Val `Debug` is available.
        let params_debug = format!("{:?}", params);

        Box::pin(async move {
            println!("[Middleware] Call -> {}::{} Inputs: {}", target, func_name, params_debug);
            let result = next(params).await;
            match &result {
                Ok(vals) => println!("[Middleware] Return <- {}::{} Outputs: {:?}", target, func_name, vals),
                Err(e) => println!("[Middleware] Error <- {}::{} Error: {:?}", target, func_name, e),
            }
            result
        })
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub struct NoOpMiddleware;
impl Middleware for NoOpMiddleware {
    fn handle(
        &self,
        _ctx: &CallContext,
        params: Vec<Val>,
        next: Next,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<Val>>> + Send>> {
        next(params)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub fn get_middleware_by_name(name: &str) -> Option<Arc<dyn Middleware>> {
    match name {
        "logging" => Some(Arc::new(LoggingMiddleware)),
        // "policy" => ...
        _ => None,
    }
}
