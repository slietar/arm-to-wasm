use wasmtime::*;

#[derive(Debug)]
struct ExitError {
    code: u32,
}

impl std::fmt::Display for ExitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ExitError with code: {}", self.code)
    }
}

impl std::error::Error for ExitError {}

fn main() -> wasmtime::Result<()> {
    let engine = Engine::default();

    let bytes = std::fs::read("../output.wasm")?;
    let module = Module::new(&engine, bytes)?;

    let mut linker = Linker::new(&engine);

    // Syscalls: https://arm64.syscall.sh/
    linker.func_wrap("ref", "supervisor_call", |mut caller: Caller<'_, ()>, param: i32, x8: i64, x0: i64, x1: i64, x2: i64, x3: i64, x4: i64, x5: i64| -> wasmtime::Result<(i64, i64)> {
        match x8 {
            0x40 => {
                let fd = x0;
                let ptr = x1 as u32;
                let len = x2 as u32;

                let memory = caller.get_export("memory").unwrap().into_memory().unwrap();

                let data = memory.data(&caller);
                let slice = &data[(ptr as usize)..((ptr + len) as usize)];

                eprintln!("Writing to fd {}: {:?}", fd, std::str::from_utf8(slice).unwrap());
            },
            0x5d => {
                eprintln!("Exit called with code: {}", x0);
                return Err(ExitError { code: x0 as u32 }.into());
            },
            _ => {
                eprint!("syscall_handler called with param: {}, x8: {}, x0: {}, x1: {}, x2: {}, x3: {}, x4: {}, x5: {}\n", param, x8, x0, x1, x2, x3, x4, x5);
            }
        }

        Ok((0, 0))
    })?;

    let mut store: Store<_> = Store::new(&engine, ());

    // Instantiation of a module requires specifying its imports and then
    // afterwards we can fetch exports by name, as well as asserting the
    // type signature of the function with `get_typed_func`.
    let instance = linker.instantiate(&mut store, &module)?;
    let hello = instance.get_typed_func::<(), ()>(&mut store, "_entry")?;

    // And finally we can call the wasm!
    let result = hello.call(&mut store, ());

    match result {
        Ok(()) => {
            eprintln!("-> WASM returned");
        },
        Err(e) if e.downcast_ref::<ExitError>().is_some() => {
            let exit_error = e.downcast_ref::<ExitError>().unwrap();
            eprintln!("-> WASM execution interrupted (exit called) with code: {}", exit_error.code);
        },
        Err(e) => {
            return Err(e);
        }
    }

    Ok(())
}
