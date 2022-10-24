use core::ptr;
use sc::syscall;
use Allocator;

/// System setting for Linux
pub struct System {
    _priv: (),
}

impl System {
    pub const fn new() -> System {
        System { _priv: () }
    }
}

#[cfg(feature = "global")]
static mut LOCK: libc::pthread_mutex_t = libc::PTHREAD_MUTEX_INITIALIZER;

unsafe impl Allocator for System {
    fn alloc(&self, size: usize) -> (*mut u8, usize, u32) {
        let addr = unsafe {
            syscall!(MMAP, 0, size, 2 | 1, 0x0020 | 0x0002, -1isize, 0) as isize
        };
        if addr < 0 {
            (ptr::null_mut(), 0, 0)
        } else {
            (addr as *mut u8, size, 0)
        }
    }

    #[cfg(target_os = "linux")]
    fn remap(&self, ptr: *mut u8, oldsize: usize, newsize: usize, can_move: bool) -> *mut u8 {
        let flags = if can_move { 1 } else { 0 };
        let ptr = unsafe {
            syscall!(MREMAP, ptr, oldsize, newsize, flags) as isize
        };
        if ptr < 0 {
            ptr::null_mut()
        } else {
            ptr as *mut u8
        }

    }

    #[cfg(target_os = "macos")]
    fn remap(&self, _ptr: *mut u8, _oldsize: usize, _newsize: usize, _can_move: bool) -> *mut u8 {
        ptr::null_mut()
    }

    #[cfg(target_os = "linux")]
    fn free_part(&self, ptr: *mut u8, oldsize: usize, newsize: usize) -> bool {
        unsafe {
            let rc = syscall!(MREMAP, ptr, oldsize, newsize, 0) as isize;
            if rc >= 0 {
                return true;
            }

            syscall!(MUNMAP, ptr.offset(newsize as isize), oldsize - newsize) == 0
        }
    }

    #[cfg(target_os = "macos")]
    fn free_part(&self, ptr: *mut u8, oldsize: usize, newsize: usize) -> bool {
        unsafe { libc::munmap(ptr.offset(newsize as isize) as *mut _, oldsize - newsize) == 0 }
    }

    fn free(&self, ptr: *mut u8, size: usize) -> bool {
        unsafe {
            syscall!(MUNMAP, ptr, size) == 0
        }
    }

    fn can_release_part(&self, _flags: u32) -> bool {
        true
    }

    fn allocates_zeros(&self) -> bool {
        true
    }

    fn page_size(&self) -> usize {
        4096
    }
}

#[cfg(feature = "global")]
pub fn acquire_global_lock() {
    unsafe { assert_eq!(libc::pthread_mutex_lock(&mut LOCK), 0) }
}

#[cfg(feature = "global")]
pub fn release_global_lock() {
    unsafe { assert_eq!(libc::pthread_mutex_unlock(&mut LOCK), 0) }
}

#[cfg(feature = "global")]
/// allows the allocator to remain unsable in the child process,
/// after a call to `fork(2)`
///
/// #Safety
///
/// if used, this function must be called,
/// before any allocations are made with the global allocator.
pub unsafe fn enable_alloc_after_fork() {
    // atfork must only be called once, to avoid a deadlock,
    // where the handler attempts to acquire the global lock twice
    static mut FORK_PROTECTED: bool = false;

    unsafe extern "C" fn _acquire_global_lock() {
        acquire_global_lock()
    }

    unsafe extern "C" fn _release_global_lock() {
        release_global_lock()
    }

    acquire_global_lock();
    // if a process forks,
    // it will acquire the lock before any other thread,
    // protecting it from deadlock,
    // due to the child being created with only the calling thread.
    if !FORK_PROTECTED {
        libc::pthread_atfork(
            Some(_acquire_global_lock),
            Some(_release_global_lock),
            Some(_release_global_lock),
        );
        FORK_PROTECTED = true;
    }
    release_global_lock();
}
