use anyhow::{Context, Result, anyhow};
use pypes_analyser::Workflow;
use std::collections::HashMap;
use wasmtime::component::{Instance, Val};
use wasmtime::Store;
use regex::Regex;
use serde_json::Value;

use crate::HostState;

pub async fn execute(
    mut store: &mut Store<HostState>,
    instances: &HashMap<String, Instance>,
    workflow: &Workflow,
) -> Result<()> {
    let mut step_outputs: HashMap<String, Value> = HashMap::new();
    let re = Regex::new(r"\{\{\s*([a-zA-Z0-9_.]+)\s*\}\}").unwrap();

    println!("\n🚀 Starting Declarative Workflow Execution...\n");

    for step in &workflow.steps {
        println!("▶ Step '{}': Calling {}.{}", step.id, step.component, step.function);

        // 1. Get Instance
        let instance = instances.get(&step.component)
            .ok_or_else(|| anyhow!("Component not found: {}", step.component))?;
        
        // 2. Get Function
        // Note: This relies on the function being exported directly.
        // If it's nested in an interface instance export, we need to handle that.
        // e.g. "calendar-api#get-free-slots" vs "get-free-slots"
        // For this POC, we check direct export first, then try some known interfaces?
        // Or just assume simple exports for now as the user example suggested.
        
        let func = if step.function.contains('.') {
            let parts: Vec<&str> = step.function.splitn(2, '.').collect();
            let interface_name = parts[0];
            let func_name = parts[1];
            
             let mut exports = instance.exports(&mut store);
             if let Some(mut exported_instance) = exports.instance(interface_name) {
                 exported_instance.func(func_name)
             } else {
                 None
             }
        } else {
             instance.get_func(&mut store, &step.function)
        }
            .ok_or_else(|| anyhow!("Function '{}' not found in component '{}' (checked root and all exports)", step.function, step.component))?;

        // 3. Prepare Arguments
        let mut args = Vec::new();
        
        // Inspect function signature to know what it expects?
        // func.params(&store) -> &[Type]
        // But dynamic construction of complex types (Records) from JSON is hard without type info.
        // FOR POC: We assume functions take 0 args OR 1 String arg.
        // This covers get-free-slots() and predict-state(String).
        
        let param_types = func.params(&store);
        
        // Match parameters
        // POC Simplification: We attempt to fill arguments by matching types
        // or using the single input template if provided.
        
        if let Some(input_template) = &step.input {
            // Templating logic (String based)
            let process_interpolation = |caps: &regex::Captures| {
                let path = &caps[1]; // e.g. "step1.output"
                let parts: Vec<&str> = path.split('.').collect();
                if parts.len() == 2 && parts[1] == "output" {
                    let step_id = parts[0];
                    if let Some(val) = step_outputs.get(step_id) {
                         // If the target expects a specific complex type, we might want to return the raw JSON
                         // But for regex replacement, we only produce a String.
                         // This path is for "Prompt Construction" mainly.
                         if let Value::String(s) = val {
                            return s.clone();
                         } else {
                            return val.to_string();
                         }
                    }
                }
                format!("UNRESOLVED({})", path)
            };

            let input_string = re.replace_all(input_template, process_interpolation).to_string();
            
            // If function takes 1 arg of type String, pass it.
            if param_types.len() == 1 {
                 if matches!(param_types[0], wasmtime::component::Type::String) {
                      args.push(Val::String(input_string.into()));
                 } else {
                      // Try to parse string as JSON to fit type?
                      if let Ok(json_val) = serde_json::from_str::<Value>(&input_string) {
                          args.push(json_to_val(&json_val, &param_types[0])?);
                      } else {
                          // Treat entire interpolated string as a string value (fallback)
                          // But if type mismatch, it will fail later or here.
                          return Err(anyhow!("Type mismatch: Step '{}' template produced a string, but function expects {:?}", step.id, param_types[0]));
                      }
                 }
            } else {
                 return Err(anyhow!("Templating supported only for single-argument functions (for now)"));
            }

        } else {
             // No template. Check if we can map previous headers/outputs automatically?
             // Or just default to empty/none.
             if param_types.is_empty() {
                 // No args needed
             } else {
                 // If 1 arg, try to use previous step output if it matches? 
                 // (Not implemented in POC, unsafe assumption).
                 // Just push default/empty
                 for ty in param_types.iter() {
                     // Try to construct a "Default" val for type?
                     // Or Error.
                     return Err(anyhow!("Function expects arguments but no input mapping provided for step '{}'", step.id));
                 }
             }
        }

        // 4. Call Function
        // allocate space for results
        let result_types = func.results(&store);
        let mut results = vec![Val::Bool(false); result_types.len()]; // Placeholder values
        
        func.call_async(&mut store, &args, &mut results).await
            .context(format!("Failed to call {}.{}", step.component, step.function))?;
        
        // 5. Capture Output
        if let Some(val) = results.first() {
            // Get the type of the first result
            let ty = &result_types[0];
            let json_val = val_to_json(val, ty, store);
            println!("  ↩ Output: {}", json_val);
            step_outputs.insert(step.id.clone(), json_val);
        } else {
            println!("  ↩ (No Output)");
        }
    }
    
    println!("\n✅ Workflow Complete.\n");
    Ok(())
}

fn val_to_json(val: &Val, ty: &wasmtime::component::Type, store: &Store<HostState>) -> Value {
    match (val, ty) {
        (Val::Bool(b), _) => Value::Bool(*b),
        (Val::S8(i), _) => Value::Number((*i).into()),
        (Val::U8(i), _) => Value::Number((*i).into()),
        (Val::S16(i), _) => Value::Number((*i).into()),
        (Val::U16(i), _) => Value::Number((*i).into()),
        (Val::S32(i), _) => Value::Number((*i).into()),
        (Val::U32(i), _) => Value::Number((*i).into()),
        (Val::S64(i), _) => Value::Number((*i).into()),
        (Val::U64(i), _) => Value::Number((*i).into()),
        (Val::Float32(f), _) => Value::Number(serde_json::Number::from_f64(*f as f64).unwrap_or(serde_json::Number::from_f64(0.0).unwrap())),
        (Val::Float64(f), _) => Value::Number(serde_json::Number::from_f64(*f).unwrap_or(serde_json::Number::from_f64(0.0).unwrap())),
        (Val::Char(c), _) => Value::String(c.to_string()),
        (Val::String(s), _) => Value::String(s.to_string()),
        (Val::List(l), wasmtime::component::Type::List(list_ty)) => {
             let element_ty = list_ty.ty();
             let values: Vec<Value> = l.iter().map(|v| val_to_json(v, &element_ty, store)).collect();
             Value::Array(values)
        },
        (Val::Record(rec), wasmtime::component::Type::Record(record_ty)) => {
            let mut map = serde_json::Map::new();
            // Record.fields() yields (&str, &Val)
            for (name, val) in rec.fields() {
                // Find the corresponding type
                if let Some(field) = record_ty.fields().find(|f| f.name == name) {
                    map.insert(name.to_string(), val_to_json(val, &field.ty, store));
                }
            }
            Value::Object(map)
        },
        (Val::Tuple(tup), wasmtime::component::Type::Tuple(tuple_ty)) => {
             let types: Vec<_> = tuple_ty.types().collect();
             let json_values: Vec<Value> = tup.values().iter().zip(types.iter())
                .map(|(v, t)| val_to_json(v, t, store))
                .collect();
             Value::Array(json_values)
        },
        (Val::Variant(v), wasmtime::component::Type::Variant(variant_ty)) => {
             let discriminant_name = v.discriminant(); 
             let mut map = serde_json::Map::new();
             if let Some(case) = variant_ty.cases().find(|c| c.name == discriminant_name) {
                 map.insert("tag".to_string(), Value::String(discriminant_name.to_string()));
                 if let Some(payload) = v.payload() {
                     if let Some(ty) = &case.ty {
                         map.insert("val".to_string(), val_to_json(payload, ty, store));
                     }
                 }
                 Value::Object(map)
             } else {
                 Value::String(format!("unknown_variant_case({})", discriminant_name))
             }
        },
        (Val::Enum(e), _) => Value::String(e.discriminant().to_string()),
        (Val::Option(o), wasmtime::component::Type::Option(option_ty)) => {
             match o.value() {
                Some(v) => val_to_json(v, &option_ty.ty(), store),
                None => Value::Null,
             }
        },
        (Val::Result(r), wasmtime::component::Type::Result(result_ty)) => {
            match r.value() {
                Ok(opt) => {
                    let map = if let Some(v) = opt {
                         if let Some(ok_ty) = result_ty.ok() {
                             let val = val_to_json(v, &ok_ty, store);
                             serde_json::Map::from_iter(vec![("ok".to_string(), val)])
                         } else {
                             serde_json::Map::from_iter(vec![("ok".to_string(), Value::Null)])
                         }
                    } else {
                         serde_json::Map::from_iter(vec![("ok".to_string(), Value::Null)])
                    };
                    Value::Object(map)
                },
                Err(opt) => {
                    let map = if let Some(v) = opt {
                         if let Some(err_ty) = result_ty.err() {
                             let val = val_to_json(v, &err_ty, store);
                             serde_json::Map::from_iter(vec![("err".to_string(), val)])
                         } else {
                             serde_json::Map::from_iter(vec![("err".to_string(), Value::Null)])
                         }
                    } else {
                         serde_json::Map::from_iter(vec![("err".to_string(), Value::Null)])
                    };
                    Value::Object(map)
                }
            }
        },
        (Val::Flags(f), _) => Value::Array(f.flags().map(|s| Value::String(s.to_string())).collect()),
        (v, t) => Value::String(format!("match_mismatch({:?}, {:?})", v, t)),
    }
}

fn json_to_val(json: &Value, ty: &wasmtime::component::Type) -> Result<Val> {
    use wasmtime::component::Type;
    match ty {
        Type::Bool => Ok(Val::Bool(json.as_bool().ok_or_else(|| anyhow!("Expected bool"))?)),
        Type::S8 => Ok(Val::S8(json.as_i64().ok_or_else(|| anyhow!("Expected number"))? as i8)),
        Type::U8 => Ok(Val::U8(json.as_u64().ok_or_else(|| anyhow!("Expected number"))? as u8)),
        Type::S16 => Ok(Val::S16(json.as_i64().ok_or_else(|| anyhow!("Expected number"))? as i16)),
        Type::U16 => Ok(Val::U16(json.as_u64().ok_or_else(|| anyhow!("Expected number"))? as u16)),
        Type::S32 => Ok(Val::S32(json.as_i64().ok_or_else(|| anyhow!("Expected number"))? as i32)),
        Type::U32 => Ok(Val::U32(json.as_u64().ok_or_else(|| anyhow!("Expected number"))? as u32)),
        Type::S64 => Ok(Val::S64(json.as_i64().ok_or_else(|| anyhow!("Expected number"))?)),
        Type::U64 => Ok(Val::U64(json.as_u64().ok_or_else(|| anyhow!("Expected number"))?)),
        Type::Float32 => Ok(Val::Float32(json.as_f64().ok_or_else(|| anyhow!("Expected number"))? as f32)),
        Type::Float64 => Ok(Val::Float64(json.as_f64().ok_or_else(|| anyhow!("Expected number"))?)),
        Type::Char => Ok(Val::Char(json.as_str().ok_or_else(|| anyhow!("Expected char string"))?.chars().next().unwrap())),
        Type::String => Ok(Val::String(json.as_str().unwrap_or(&json.to_string()).to_string().into())),
        Type::List(list_ty) => {
            let arr = json.as_array().ok_or_else(|| anyhow!("Expected array"))?;
            let elem_ty = list_ty.ty();
            let vals: Result<Vec<Val>> = arr.iter().map(|v| json_to_val(v, &elem_ty)).collect();
            let vals_vec = vals?;
            Ok(list_ty.new_val(vals_vec.into_boxed_slice())?)
        },
        Type::Record(record_ty) => {
            let obj = json.as_object().ok_or_else(|| anyhow!("Expected object"))?;
            let mut values = Vec::new();
            for field in record_ty.fields() {
                let v = obj.get(field.name).ok_or_else(|| anyhow!("Missing field '{}'", field.name))?;
                values.push((field.name, json_to_val(v, &field.ty)?));
            }
            Ok(record_ty.new_val(values)?)
        },
        Type::Tuple(tuple_ty) => {
             let arr = json.as_array().ok_or_else(|| anyhow!("Expected array for tuple"))?;
             let mut values = Vec::new();
             for (json_v, ty) in arr.iter().zip(tuple_ty.types()) {
                 values.push(json_to_val(json_v, &ty)?);
             }
             Ok(tuple_ty.new_val(values.into_boxed_slice())?)
        },
        Type::Variant(variant_ty) => {
            let obj = json.as_object().ok_or_else(|| anyhow!("Expected object for variant"))?;
            let tag = obj.get("tag").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("Missing 'tag' in variant object"))?;
            
            let case = variant_ty.cases().find(|c| c.name == tag)
                .ok_or_else(|| anyhow!("Unknown variant tag '{}'", tag))?;
            
            let payload = if let Some(payload_ty) = &case.ty {
                let val_json = obj.get("val").ok_or_else(|| anyhow!("Missing 'val' for variant payload"))?;
                Some(json_to_val(val_json, payload_ty)?)
            } else {
                None
            };
            
            Ok(variant_ty.new_val(tag, payload)?)
        },
        Type::Option(option_ty) => {
            if json.is_null() {
                Ok(option_ty.new_val(None)?)
            } else {
                let inner = json_to_val(json, &option_ty.ty())?;
                Ok(option_ty.new_val(Some(inner))?)
            }
        },
        _ => Err(anyhow!("Unsupported type for json_to_val: {:?}", ty)),
    }
}
