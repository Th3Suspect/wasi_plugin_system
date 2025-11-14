use anyhow::{Context, Ok, Result};
use plugin_api::{PluginInfo, PluginRequest, PluginResponse};
use std::path::Path;
use wasmtime::{
    Caller, Config, Engine, Instance, Linker, Memory, Module, Store, WasmBacktraceDetails,
};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder};

struct WasmPlugin {
    store: Store<WasiCtx>,
    instance: Instance,
    memory: Memory,
    info: PluginInfo,
}

impl WasmPlugin {
    fn new(engine: &Engine, wasm_path: impl AsRef<Path>) -> Result<Self> {
        let module = Module::from_file(engine, wasm_path)?;
        let wasi = WasiCtxBuilder::new().inherit_stdio().inherit_args().build();
        let mut store = Store::new(engine, wasi);

        let _wasi = WasiCtxBuilder::new().inherit_stdio().inherit_args().build();

        let mut linker = Linker::new(engine);

        linker.func_wrap(
            "wasi_snapshot_preview1",
            "fd_write",
            move |mut _caller: Caller<'_, WasiCtx>,
                  fd: i32,
                  iovs_ptr: i32,
                  iovs_len: i32,
                  nwritten_ptr: i32|
                  -> i32 {
                println!(
                    "fd_write called: fd={}, iovs_ptr={}, iovs_len={}, nwritten_ptr={}",
                    fd, iovs_ptr, iovs_len, nwritten_ptr
                );
                0
            },
        )?;

        linker.func_wrap(
            "wasi_snapshot_preview1",
            "environ_sizes_get",
            move |_caller: Caller<'_, WasiCtx>,
                  _environc_ptr: i32,
                  _environ_buf_size_ptr: i32|
                  -> i32 { 0 },
        )?;

        linker.func_wrap(
            "wasi_snapshot_preview1",
            "proc_exit",
            |_caller: Caller<'_, WasiCtx>, _code: i32| -> Result<()> { Ok(()) },
        )?;

        linker.func_wrap(
            "wasi_snapshot_preview1",
            "environ_get",
            move |_caller: Caller<'_, WasiCtx>, _environ_ptrs: i32, _environ_buf: i32| -> i32 {
                0
            },
        )?;

        let instance = linker.instantiate(&mut store, &module)?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .context("Plugin should export memory")?;

        let init_func = instance.get_typed_func::<(), i32>(&mut store, "plugin_init")?;

        let ptr = init_func.call(&mut store, ())?;

        let get_len_func = instance.get_typed_func::<(), i32>(&mut store, "get_result_len")?;
        let len = get_len_func.call(&mut store, ())? as usize;

        let data = memory.data(&store);
        let json_bytes = &data[ptr as usize..(ptr as usize + len)];
        let info: PluginInfo = serde_json::from_slice(json_bytes)?;

        println!("✓ Loaded plugin: {} v{}", info.name, info.version);

        Ok(Self {
            store,
            instance,
            memory,
            info,
        })
    }

    fn execute(&mut self, request: PluginRequest) -> Result<PluginResponse> {
        let request_json = serde_json::to_vec(&request)?;
        let request_len = request_json.len();

        let ptr = match self
            .instance
            .get_typed_func::<i32, i32>(&mut self.store, "allocate")
        {
            core::result::Result::Ok(alloc) => alloc.call(&mut self.store, request_len as i32)?,
            core::result::Result::Err(_) => 1024,
        };

        let memory_data = self.memory.data_mut(&mut self.store);
        memory_data[ptr as usize..(ptr as usize) + request_len].copy_from_slice(&request_json);

        let execute_func = self
            .instance
            .get_typed_func::<(i32, i32), i32>(&mut self.store, "plugin_execute")?;

        let result_ptr = execute_func.call(&mut self.store, (ptr, request_len as i32))?;

        let get_len_func = self
            .instance
            .get_typed_func::<(), i32>(&mut self.store, "get_result_len")?;
        let result_len = get_len_func.call(&mut self.store, ())? as usize;

        let data = self.memory.data(&self.store);
        let response_bytes = &data[result_ptr as usize..(result_ptr as usize + result_len)];
        let response: PluginResponse = serde_json::from_slice(response_bytes)?;
        match self
            .instance
            .get_typed_func::<(i32, i32), ()>(&mut self.store, "deallocate")
        {
            std::result::Result::Ok(dealloc) => {
                let _ = dealloc.call(&mut self.store, (ptr, request_len as i32));
            }
            std::result::Result::Err(_) => {}
        }
        Ok(response)
    }
}

struct PluginManager {
    engine: Engine,
    plugins: Vec<WasmPlugin>,
}

impl PluginManager {
    fn new() -> Self {
        let mut config = Config::new();
        config.wasm_backtrace_details(WasmBacktraceDetails::Enable);

        Self {
            engine: Engine::new(&config).unwrap(),
            plugins: Vec::new(),
        }
    }

    fn load_plugin(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let plugin = WasmPlugin::new(&self.engine, path)?;
        self.plugins.push(plugin);
        Ok(())
    }

    fn execute_plugin(&mut self, index: usize, request: PluginRequest) -> Result<PluginResponse> {
        self.plugins[index].execute(request)
    }

    fn list_plugins(&self) {
        println!("\n Loaded plugin:");
        for (i, plugin) in self.plugins.iter().enumerate() {
            println!(
                "[{}] {} v{} by {}",
                i, plugin.info.name, plugin.info.version, plugin.info.author
            )
        }
    }
}

fn main() -> Result<()> {
    println!("Init Wasmtime plugin system\n");

    let mut manager = PluginManager::new();

    manager.load_plugin("./plugins/example_plugin.wasm")?;
    manager.list_plugins();

    let request = PluginRequest {
        command: "greet".to_string(),
        data: "World".to_string(),
    };

    let response = manager.execute_plugin(0, request)?;
    println!("Result: {:?}", response);

    let request = PluginRequest {
        command: "reverse".to_string(),
        data: "Hello, Rust!".to_string(),
    };
    let response = manager.execute_plugin(0, request)?;
    println!("Result: {:?}", response);
    Ok(())
}
