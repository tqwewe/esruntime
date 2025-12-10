/// Generates the WASM exports for a command handler.
///
/// This macro creates the extern "C" functions that the runtime calls:
///
/// - `__esruntime_event_types` - Returns the event types to query
/// - `__esruntime_domain_id_bindings` - Returns domain ID bindings from input
/// - `__esruntime_apply` - Apply an event to the handler
/// - `__esruntime_execute` - Execute the command
/// - `__esruntime_alloc` - Allocate memory for the runtime to write into
/// - `__esruntime_dealloc` - Free memory
///
/// # Usage
///
/// ```rust
/// export_handler!(MyCommandHandler);
/// ```
#[macro_export]
macro_rules! export_handler {
    ($handler:ty) => {
        // Thread-local storage for the handler instance during execution
        std::thread_local! {
            static HANDLER: std::cell::RefCell<Option<$handler>> = std::cell::RefCell::new(None);
        }

        /// Initialize a new handler instance
        #[no_mangle]
        pub extern "C" fn __esruntime_init() {
            HANDLER.with(|h| {
                *h.borrow_mut() = Some(<$handler as Default>::default());
            });
        }

        /// Get event types this handler queries (returns JSON array)
        #[no_mangle]
        pub extern "C" fn __esruntime_event_types(out_ptr: *mut u8, out_len: *mut usize) {
            let types =
                <<$handler as $crate::CommandHandler>::Query as $crate::EventSet>::event_types();
            let json = serde_json::to_vec(types).unwrap();
            unsafe {
                let len = json.len();
                std::ptr::copy_nonoverlapping(json.as_ptr(), out_ptr, len);
                *out_len = len;
            }
        }

        /// Parse input and get domain ID bindings (returns JSON)
        #[no_mangle]
        pub extern "C" fn __esruntime_domain_id_bindings(
            input_ptr: *const u8,
            input_len: usize,
            out_ptr: *mut u8,
            out_len: *mut usize,
        ) -> i32 {
            let input_bytes = unsafe { std::slice::from_raw_parts(input_ptr, input_len) };

            let input: <$handler as $crate::CommandHandler>::Input =
                match serde_json::from_slice(input_bytes) {
                    Ok(i) => i,
                    Err(_) => return -1, // Invalid input
                };

            let bindings = <_ as $crate::CommandInput>::domain_id_bindings(&input);
            let json = serde_json::to_vec(&bindings).unwrap();

            unsafe {
                let len = json.len();
                std::ptr::copy_nonoverlapping(json.as_ptr(), out_ptr, len);
                *out_len = len;
            }

            0 // Success
        }

        /// Apply an event to the handler
        #[no_mangle]
        pub extern "C" fn __esruntime_apply(
            event_type_ptr: *const u8,
            event_type_len: usize,
            event_data_ptr: *const u8,
            event_data_len: usize,
        ) -> i32 {
            let event_type = unsafe {
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    event_type_ptr,
                    event_type_len,
                ))
            };
            let event_data = unsafe { std::slice::from_raw_parts(event_data_ptr, event_data_len) };

            let event =
                match <<$handler as $crate::CommandHandler>::Query as $crate::EventSet>::from_event(
                    event_type, event_data,
                ) {
                    Some(Ok(e)) => e,
                    Some(Err(_)) => return -1, // Deserialization error
                    None => return -2,         // Unknown event type
                };

            HANDLER.with(|h| {
                if let Some(handler) = h.borrow_mut().as_mut() {
                    handler.apply(event);
                }
            });

            0 // Success
        }

        /// Execute the command (consumes handler, returns result JSON)
        #[no_mangle]
        pub extern "C" fn __esruntime_execute(
            input_ptr: *const u8,
            input_len: usize,
            out_ptr: *mut u8,
            out_len: *mut usize,
        ) -> i32 {
            let input_bytes = unsafe { std::slice::from_raw_parts(input_ptr, input_len) };

            let input: <$handler as $crate::CommandHandler>::Input =
                match serde_json::from_slice(input_bytes) {
                    Ok(i) => i,
                    Err(e) => {
                        let err = $crate::CommandError::invalid_input(e.to_string());
                        let json =
                            serde_json::to_vec(&ExecuteResultDto::Err((&err).into())).unwrap();
                        unsafe {
                            std::ptr::copy_nonoverlapping(json.as_ptr(), out_ptr, json.len());
                            *out_len = json.len();
                        }
                        return 1; // Error (but wrote result)
                    }
                };

            let handler = HANDLER.with(|h| h.borrow_mut().take()).unwrap_or_default();

            let result = handler.execute(input);

            let dto = match result {
                Ok(emit) => ExecuteResultDto::Ok(
                    emit.into_events()
                        .into_iter()
                        .map(|e| EmittedEventDto {
                            event_type: e.event_type,
                            data: e.data,
                            domain_ids: e
                                .domain_ids
                                .into_iter()
                                .map(|(k, v)| (k.to_string(), v.as_option().map(|s| s.to_string())))
                                .collect(),
                        })
                        .collect(),
                ),
                Err(e) => ExecuteResultDto::Err((&e).into()),
            };

            let json = serde_json::to_vec(&dto).unwrap();
            unsafe {
                std::ptr::copy_nonoverlapping(json.as_ptr(), out_ptr, json.len());
                *out_len = json.len();
            }

            match dto {
                ExecuteResultDto::Ok(_) => 0,
                ExecuteResultDto::Err(_) => 1,
            }
        }

        /// Allocate memory for the runtime
        #[no_mangle]
        pub extern "C" fn __esruntime_alloc(len: usize) -> *mut u8 {
            let mut buf = Vec::with_capacity(len);
            let ptr = buf.as_mut_ptr();
            std::mem::forget(buf);
            ptr
        }

        /// Free memory
        #[no_mangle]
        pub extern "C" fn __esruntime_dealloc(ptr: *mut u8, len: usize) {
            unsafe {
                drop(Vec::from_raw_parts(ptr, 0, len));
            }
        }

        // DTOs for serializing results back to the runtime
        #[derive(serde::Serialize)]
        #[serde(tag = "status", rename_all = "lowercase")]
        enum ExecuteResultDto {
            Ok(Vec<EmittedEventDto>),
            Err(CommandErrorDto),
        }

        #[derive(serde::Serialize)]
        struct EmittedEventDto {
            event_type: String,
            data: Vec<u8>,
            domain_ids: std::collections::HashMap<String, Option<String>>,
        }

        #[derive(serde::Serialize)]
        struct CommandErrorDto {
            code: String,
            message: String,
        }

        impl From<&$crate::CommandError> for CommandErrorDto {
            fn from(err: &$crate::CommandError) -> Self {
                Self {
                    code: match err.code {
                        $crate::ErrorCode::Rejected => "rejected",
                        $crate::ErrorCode::InvalidInput => "invalid_input",
                        $crate::ErrorCode::Internal => "internal",
                    }
                    .to_string(),
                    message: err.message.clone(),
                }
            }
        }
    };
}
