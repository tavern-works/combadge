#[cfg(feature = "log")]
#[macro_export]
macro_rules! log_info {
    ($($arg:expr),*) => {
        ::log::info!($($arg),*)
    }
}

#[cfg(feature = "log")]
#[macro_export]
macro_rules! log_error {
    ($($arg:expr),*) => {
        ::log::error!($($arg),*)
    }
}

#[cfg(not(feature = "log"))]
#[macro_export]
macro_rules! log_info {
    ($($arg:expr),*) => {
        {}
    }
}

#[cfg(not(feature = "log"))]
#[macro_export]
macro_rules! log_error {
    ($($arg:expr),*) => {
        {}
    }
}
