use std::sync::atomic::AtomicBool;

pub static DO_DEBUG: AtomicBool = AtomicBool::new(false);

#[macro_export]
macro_rules! debug_print {
    ( $val:expr ) => {
        // if $crate::DO_DEBUG.load(std::sync::atomic::Ordering::SeqCst) {
        //     dbg!( $val )
        // } else {
        //     ( $val )
        // }
    };
}

#[macro_export]
macro_rules! do_debug {
    () => {
        do_debug!(true)
    };
    (true) => {
        $crate::DO_DEBUG.store(true, std::sync::atomic::Ordering::SeqCst);
    };
    (false) => {
        $crate::DO_DEBUG.store(false, std::sync::atomic::Ordering::SeqCst);
    };
}

#[macro_export]
macro_rules! run_with_debug {
    { $($t:tt)* } => {
        {
            let old = $crate::DO_DEBUG.load(std::sync::atomic::Ordering::SeqCst);
            do_debug!(true);

            let v = {
                $(
                    $t
                )*
            };

            $crate::DO_DEBUG.store(old, std::sync::atomic::Ordering::SeqCst);

            v
        }
    };
}

mod test {
    #[test]
    pub fn test() {
        debug_print!("hi");

        run_with_debug!{
            debug_print!("hi 2");
        }

        debug_print!("hi 3");
    }
}