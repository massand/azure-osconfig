use std::{
    ffi::{c_int, CString},
    path::PathBuf,
    slice,
};

use anyhow::anyhow;
use serde::Deserialize;
use sharedlib::{FuncArc, LibArc, Symbol};

use crate::bindings::{
    Close, Get, Handle, Info, JsonString, Open, Set, CLOSE, GET, INFO, OK, OPEN, SET,
};

// TODO: this should be called something like "Schema"
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")] // TODO: this seems bad to be PascalCase
pub struct ModuleInfo {
    pub name: String,
    pub description: String,
    pub manufacturer: String,
    // pub version: String, // TODO: Version struct ???
    pub components: Vec<String>,
    // TODO:
    // - version info
    // - lifetime
    // - license URI
    // - project URI
    // - user account
}

/// A struct representation of a module's shared library.
#[derive(Clone)]
pub struct Library {
    // lib: LibArc,
    info: FuncArc<Info>,
    open: FuncArc<Open>,
    close: FuncArc<Close>,
    set: FuncArc<Set>,
    get: FuncArc<Get>,
}

#[derive(Clone)]
pub struct Session(Handle);

// https://stackoverflow.com/questions/60292897/why-cant-i-send-mutexmut-c-void-between-threads
unsafe impl Send for Session {}

impl Library {
    pub fn load(path: PathBuf) -> anyhow::Result<Self> {
        if path.extension().unwrap() != "so" {
            return Err(anyhow::anyhow!("Invalid module file extension"));
        }

        unsafe {
            let lib = LibArc::new(path).map_err(|err| anyhow!(err.to_string()))?;

            let info: FuncArc<Info> = lib
                .find_func(INFO)
                .map_err(|err| anyhow!(err.to_string()))?;
            let open: FuncArc<Open> = lib
                .find_func(OPEN)
                .map_err(|err| anyhow!(err.to_string()))?;
            let close: FuncArc<Close> = lib
                .find_func(CLOSE)
                .map_err(|err| anyhow!(err.to_string()))?;
            let set: FuncArc<Set> = lib.find_func(SET).map_err(|err| anyhow!(err.to_string()))?;
            let get: FuncArc<Get> = lib.find_func(GET).map_err(|err| anyhow!(err.to_string()))?;

            Ok(Self {
                // lib,
                info,
                open,
                close,
                set,
                get,
            })
        }
    }

    pub fn info(&self, client: &str) -> anyhow::Result<ModuleInfo> {
        let get_info = unsafe { self.info.get() };
        let client_name = CString::new(client).unwrap();
        let mut payload: JsonString = std::ptr::null_mut();
        let mut payload_size_bytes: c_int = 0;

        let status = get_info(client_name.as_ptr(), &mut payload, &mut payload_size_bytes);

        if status != OK {
            return Err(anyhow::anyhow!("GetInfo() failed: {}", status));
        }

        let payload =
            unsafe { slice::from_raw_parts(payload as *const u8, payload_size_bytes as usize) };
        let payload = String::from_utf8_lossy(payload).to_string();

        let info: ModuleInfo = serde_json::from_str(&payload)?;

        Ok(info)
    }

    pub fn open(&self, client: &str, max_payload_size: u32) -> anyhow::Result<Session> {
        let open = unsafe { self.open.get() };
        let client_name = CString::new(client).unwrap();

        let handle = open(client_name.as_ptr(), max_payload_size);

        if handle.is_null() {
            return Err(anyhow::anyhow!("Open() failed"));
        }

        Ok(Session(handle))
    }

    pub fn close(&self, session: Session) -> anyhow::Result<()> {
        let close = unsafe { self.close.get() };
        let handle = session.0;

        close(handle);

        Ok(())
    }

    pub fn set(
        &self,
        session: &Session,
        component: &str,
        object: &str,
        payload: &str,
        size: usize,
    ) -> anyhow::Result<i32> {
        let set = unsafe { self.set.get() };
        let handle = session.0;
        let component_name = CString::new(component).unwrap();
        let object_name = CString::new(object).unwrap();
        let value = CString::new(payload).unwrap();

        let status = set(
            handle,
            component_name.as_ptr(),
            object_name.as_ptr(),
            value.as_ptr() as JsonString,
            size as i32,
        );

        Ok(status)
    }

    pub fn get(
        &self,
        session: &Session,
        component: &str,
        object: &str,
    ) -> anyhow::Result<(i32, String)> {
        let get = unsafe { self.get.get() };
        let handle = session.0;
        let component_name = CString::new(component).unwrap();
        let object_name = CString::new(object).unwrap();
        let mut payload: JsonString = std::ptr::null_mut();
        let mut payload_size_bytes: c_int = 0;

        let status = get(
            handle,
            component_name.as_ptr(),
            object_name.as_ptr(),
            &mut payload,
            &mut payload_size_bytes,
        );

        let payload =
            unsafe { slice::from_raw_parts(payload as *const u8, payload_size_bytes as usize) };
        let payload = String::from_utf8_lossy(payload).to_string();

        Ok((status, payload))
    }
}
