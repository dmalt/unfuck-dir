use std::{env, ffi, sync};

static ENV_LOCK: sync::Mutex<()> = sync::Mutex::new(());

struct VarsGuard {
    vars: Vec<VarGuard>,
    _lock: sync::MutexGuard<'static, ()>,
}

struct VarGuard {
    key: String,
    old: Option<ffi::OsString>,
}

impl Drop for VarGuard {
    fn drop(&mut self) {
        match &self.old {
            Some(v) => unsafe { env::set_var(&self.key, v) },
            None => unsafe { env::remove_var(&self.key) },
        };
    }
}

pub fn with_var<F, R>(key: &str, value: Option<&str>, f: F) -> R
where
    F: FnOnce() -> R,
{
    with_vars(&[(key, value)], f)
}

pub fn with_vars<F, R>(vars: &[(&str, Option<&str>)], f: F) -> R
where
    F: FnOnce() -> R,
{
    let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut guard = VarsGuard {
        vars: Vec::new(),
        _lock: lock,
    };
    for (key, value) in vars.iter() {
        let guard_single = VarGuard {
            key: key.to_string(),
            old: env::var_os(key),
        };
        guard.vars.push(guard_single);
        match value {
            Some(v) => unsafe { env::set_var(*key, *v) },
            None => unsafe { env::remove_var(*key) },
        };
    }
    f()
}
