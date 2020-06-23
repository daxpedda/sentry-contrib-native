#![warn(clippy::all, clippy::nursery, clippy::pedantic, missing_docs)]
#![cfg_attr(
    feature = "nightly",
    feature(external_doc),
    doc(include = "../README.md")
)]
#![cfg_attr(not(feature = "nightly"), doc = "")]

use std::{
    fmt::Debug,
    os::raw::{c_char, c_int, c_void},
};
pub use va_list::VaList;

#[allow(non_camel_case_types)]
type c_wchar = u16;

/// The Sentry Client Options.
///
/// See <https://docs.sentry.io/error-reporting/configuration/>
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Options([u8; 0]);

/// Represents a Sentry protocol value.
///
/// The members of this type should never be accessed.  They are only here
/// so that alignment for the type can be properly determined.
///
/// Values must be released with `sentry_value_decref`.  This lowers the
/// internal refcount by one.  If the refcount hits zero it's freed.  Some
/// values like primitives have no refcount (like null) so operations on
/// those are no-ops.
///
/// In addition values can be frozen.  Some values like primitives are always
/// frozen but lists and dicts are not and can be frozen on demand.  This
/// automatically happens for some shared values in the event payload like
/// the module list.
#[repr(C)]
#[derive(Copy, Clone)]
pub union Value {
    _bits: u64,
    _double: f64,
    _bindgen_union_align: u64,
}

/// A UUID
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Uuid {
    /// Bytes of the uuid.
    pub bytes: [c_char; 16],
}

/// Sentry levels for events and breadcrumbs.
#[repr(i32)]
pub enum Level {
    /// Debug
    Debug = -1,
    /// Info
    Info = 0,
    /// Warning
    Warning = 1,
    /// Error
    Error = 2,
    /// Fatal
    Fatal = 3,
}

/// Type of a Sentry value.
#[repr(i32)]
pub enum ValueType {
    /// Null
    Null = 0,
    /// Bool
    Bool = 1,
    /// Integer
    Int = 2,
    /// Double
    Double = 3,
    /// String
    String = 4,
    /// List
    List = 5,
    /// Object
    Object = 6,
}

/// The state of user consent.
#[repr(i32)]
pub enum UserConsent {
    /// Unknown
    Unknown = -1,
    /// Given
    Given = 1,
    /// Revoked
    Revoked = 0,
}

/// This represents an interface for user-defined transports.
///
/// Transports are responsible for sending envelopes to sentry and are the last
/// step in the event pipeline. A transport has the following hooks, all of which
/// take the user provided `state` as last parameter. The transport state needs
/// to be set with `sentry_transport_set_state` and typically holds handles and
/// other information that can be reused across requests.
///
/// * `send_func`: This function will take ownership of an envelope, and is
///   responsible for freeing it via `sentry_envelope_free`.
/// * `startup_func`: This hook will be called by sentry and instructs the
///   transport to initialize itself.
/// * `shutdown_func`: Instructs the transport to flush its queue and shut down.
///   This hook receives a millisecond-resolution `timeout` parameter and should
///   return `true` when the transport was flushed and shut down successfully.
///   In case of `false`, sentry will log an error, but continue with freeing the
///   transport.
/// * `free_func`: Frees the transports `state`. This hook might be called even
///   though `shudown_func` returned `false` previously.
///
/// The transport interface might be extended in the future with hooks to flush
/// its internal queue without shutting down, and to dump its internal queue to
/// disk in case of a hard crash.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Transport([u8; 0]);

/// A Sentry Envelope.
///
/// The Envelope is an abstract type which represents a payload being sent to
/// sentry. It can contain one or more items, typically an Event.
/// See <https://develop.sentry.dev/sdk/envelopes/>
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Envelope([u8; 0]);

/// Type of the callback for modifying events.
pub type EventFunction =
    extern "C" fn(event: Value, hint: *mut c_void, closure: *mut c_void) -> Value;

/// Type of the callback for logging debug events.
pub type LoggerFunction = extern "C" fn(level: i32, message: *const c_char, args: VaList);

/// Type of callback for sending envelopes to a Sentry service
pub type SendEnvelopeFunction = extern "C" fn(envelope: *mut Envelope, state: *mut c_void);

/// Type of the callback for starting up a custom transport
pub type StartupFunction = extern "C" fn(options: *const Options, state: *mut c_void);

/// Type of the callback for shutting down a custom transport
pub type ShutdownFunction = extern "C" fn(timeout: u64, state: *mut c_void) -> bool;

extern "C" {
    /// Releases memory allocated from the underlying allocator.
    #[link_name = "sentry_free"]
    pub fn free(ptr: *mut c_void);

    /// Increments the reference count on the value.
    #[link_name = "sentry_value_incref"]
    pub fn value_incref(value: Value);

    /// Decrements the reference count on the value.
    #[link_name = "sentry_value_decref"]
    pub fn value_decref(value: Value);

    /// Creates a null value.
    #[link_name = "sentry_value_new_null"]
    pub fn value_new_null() -> Value;

    /// Creates a new 32-bit signed integer value.
    #[link_name = "sentry_value_new_int32"]
    pub fn value_new_int32(value: i32) -> Value;

    /// Creates a new double value.
    #[link_name = "sentry_value_new_double"]
    pub fn value_new_double(value: f64) -> Value;

    /// Creates a new boolen value.
    #[link_name = "sentry_value_new_bool"]
    pub fn value_new_bool(value: c_int) -> Value;

    /// Creates a new null terminated string.
    #[link_name = "sentry_value_new_string"]
    pub fn value_new_string(value: *const c_char) -> Value;

    /// Creates a new list value.
    #[link_name = "sentry_value_new_list"]
    pub fn value_new_list() -> Value;

    /// Creates a new object.
    #[link_name = "sentry_value_new_object"]
    pub fn value_new_object() -> Value;

    /// Returns the type of the value passed.
    #[link_name = "sentry_value_get_type"]
    pub fn value_get_type(value: Value) -> ValueType;

    /// Sets a key to a value in the map.
    ///
    /// This moves the ownership of the value into the map.  The caller does not
    /// have to call `sentry_value_decref` on it.
    #[link_name = "sentry_value_set_by_key"]
    pub fn value_set_by_key(value: Value, k: *const c_char, v: Value) -> c_int;

    /// This removes a value from the map by key.
    #[link_name = "sentry_value_remove_by_key"]
    pub fn value_remove_by_key(value: Value, k: *const c_char) -> c_int;

    /// Appends a value to a list.
    /// This moves the ownership of the value into the list.  The caller does
    /// not have to call `sentry_value_decref` on it.
    #[link_name = "sentry_value_append"]
    pub fn value_append(value: Value, v: Value) -> c_int;

    /// Inserts a value into the list at a certain position.
    ///
    /// This moves the ownership of the value into the list.  The caller does
    /// not have to call `sentry_value_decref` on it.
    ///
    /// If the list is shorter than the given index it's automatically extended
    /// and filled with `null` values.
    #[link_name = "sentry_value_set_by_index"]
    pub fn value_set_by_index(value: Value, index: usize, v: Value) -> c_int;

    /// This removes a value from the list by index.
    #[link_name = "sentry_value_remove_by_index"]
    pub fn value_remove_by_index(value: Value, index: usize) -> c_int;

    /// Looks up a value in a map by key.  If missing a null value is
    /// returned."] The returned value is borrowed."]
    #[link_name = "sentry_value_get_by_key"]
    pub fn value_get_by_key(value: Value, k: *const c_char) -> Value;

    /// Looks up a value in a map by key.  If missing a null value is returned.
    /// The returned value is owned.
    ///
    /// If the caller no longer needs the value it must be released with
    /// `sentry_value_decref`.
    #[link_name = "sentry_value_get_by_key_owned"]
    pub fn value_get_by_key_owned(value: Value, k: *const c_char) -> Value;

    /// Looks up a value in a list by index.  If missing a null value is
    /// returned. The returned value is borrowed.
    #[link_name = "sentry_value_get_by_index"]
    pub fn value_get_by_index(value: Value, index: usize) -> Value;

    /// Looks up a value in a list by index.  If missing a null value is
    /// returned. The returned value is owned.
    ///
    /// If the caller no longer needs the value it must be released with
    /// `sentry_value_decref`.
    #[link_name = "sentry_value_get_by_index_owned"]
    pub fn value_get_by_index_owned(value: Value, index: usize) -> Value;

    /// Returns the length of the given map or list.
    ///
    /// If an item is not a list or map the return value is 0.
    #[link_name = "sentry_value_get_length"]
    pub fn value_get_length(value: Value) -> usize;

    /// Converts a value into a 32bit signed integer.
    #[link_name = "sentry_value_as_int32"]
    pub fn value_as_int32(value: Value) -> i32;

    /// Converts a value into a double value.
    #[link_name = "sentry_value_as_double"]
    pub fn value_as_double(value: Value) -> f64;

    /// Returns the value as c string.
    #[link_name = "sentry_value_as_string"]
    pub fn value_as_string(value: Value) -> *const c_char;

    /// Returns `true` if the value is boolean true.
    #[link_name = "sentry_value_is_true"]
    pub fn value_is_true(value: Value) -> c_int;

    /// Creates a new empty event value.
    #[link_name = "sentry_value_new_event"]
    pub fn value_new_event() -> Value;

    /// Creates a new message event value.
    ///
    /// `logger` can be NULL to omit the logger value.
    #[link_name = "sentry_value_new_message_event"]
    pub fn value_new_message_event(level: i32, logger: *const c_char, text: *const c_char)
        -> Value;

    /// Creates a new breadcrumb with a specific type and message.
    ///
    /// Either parameter can be NULL in which case no such attributes is
    /// created.
    #[link_name = "sentry_value_new_breadcrumb"]
    pub fn value_new_breadcrumb(type_: *const c_char, message: *const c_char) -> Value;

    /// Serialize a Sentry value to msgpack.
    ///
    /// The string is freshly allocated and must be freed with
    /// `sentry_string_free`.  Since msgpack is not zero terminated
    /// the size is written to the `size_out` parameter.
    #[link_name = "sentry_value_to_msgpack"]
    pub fn value_to_msgpack(value: Value, size_out: *mut usize) -> *mut c_char;

    /// Adds a stacktrace to an event.
    ///
    /// If `ips` is NULL the current stacktrace is captured, otherwise `len`
    /// stacktrace instruction pointers are attached to the event.
    #[link_name = "sentry_event_value_add_stacktrace"]
    pub fn event_value_add_stacktrace(event: Value, ips: *mut *mut c_void, len: usize);

    /// Creates the nil uuid.
    #[link_name = "sentry_uuid_nil"]
    pub fn uuid_nil() -> Uuid;

    /// Formats the uuid into a string buffer.
    #[link_name = "sentry_uuid_as_string"]
    pub fn uuid_as_string(uuid: *const Uuid, str: *mut c_char);

    /// Frees an envelope.
    #[link_name = "sentry_envelope_free"]
    pub fn envelope_free(envelope: *mut Envelope);

    /// Serializes the envelope.
    ///
    /// The return value needs to be freed with `sentry_string_free()`.
    #[link_name = "sentry_envelope_serialize"]
    pub fn envelope_serialize(envelope: *const Envelope, size: &mut usize) -> *const c_char;

    /// Creates a new transport with an initial `send_func`.
    #[link_name = "sentry_transport_new"]
    pub fn transport_new(send_fn: SendEnvelopeFunction) -> *mut Transport;

    /// Sets the transport `state`.
    ///
    /// If the state is owned by the transport and needs to be freed, use
    /// `transport_set_free_func` to set an appropriate hook.
    #[link_name = "sentry_transport_set_state"]
    pub fn transport_set_state(transport: *mut Transport, state: *mut c_void);

    /// Sets the transport hook to free the transport `state`.
    #[link_name = "sentry_transport_set_free_func"]
    pub fn transport_set_free_func(
        transport: *mut Transport,
        free_fn: extern "C" fn(state: *mut c_void),
    );

    /// Sets the transport startup hook.
    #[link_name = "sentry_transport_set_startup_func"]
    pub fn transport_set_startup_hook(transport: *mut Transport, startup_fn: StartupFunction);

    /// Sets the transport shutdown hook.
    ///
    /// This hook will receive a millisecond-resolution timeout; it should return
    /// `true` in case all the pending envelopes have been sent within the timeout,
    /// or `false` if the timeout was hit.
    #[link_name = "sentry_transport_set_shutdown_func"]
    pub fn transport_set_shutdown_func(transport: *mut Transport, shutdown_fn: ShutdownFunction);

    /// Generic way to free a transport.
    #[link_name = "sentry_transport_free"]
    pub fn transport_free(transport: *mut Transport);

    /// Creates a new options struct.
    /// Can be freed with `sentry_options_free`.
    #[link_name = "sentry_options_new"]
    pub fn options_new() -> *mut Options;

    /// Deallocates previously allocated Sentry options.
    #[link_name = "sentry_options_free"]
    pub fn options_free(opts: *mut Options);

    /// Sets a transport.
    #[link_name = "sentry_options_set_transport"]
    pub fn options_set_transport(opts: *mut Options, transport: *mut Transport);

    /// Sets the before send callback.
    #[link_name = "sentry_options_set_before_send"]
    pub fn options_set_before_send(
        opts: *mut Options,
        func: Option<EventFunction>,
        data: *mut c_void,
    );

    /// Sets the DSN.
    #[link_name = "sentry_options_set_dsn"]
    pub fn options_set_dsn(opts: *mut Options, dsn: *const c_char);

    /// Gets the DSN.
    #[link_name = "sentry_options_get_dsn"]
    pub fn options_get_dsn(opts: *const Options) -> *const c_char;

    /// Sets the sample rate, which should be a double between `0.0` and `1.0`.
    /// Sentry will randomly discard any event that is captured using
    /// `sentry_capture_event` when a sample rate < 1 is set.
    #[link_name = "sentry_options_set_sample_rate"]
    pub fn options_set_sample_rate(opts: *mut Options, sample_rate: f64);

    /// Gets the sample rate.
    #[link_name = "sentry_options_get_sample_rate"]
    pub fn options_get_sample_rate(opts: *const Options) -> f64;

    /// Sets the release.
    #[link_name = "sentry_options_set_release"]
    pub fn options_set_release(opts: *mut Options, release: *const c_char);

    /// Gets the release.
    #[link_name = "sentry_options_get_release"]
    pub fn options_get_release(opts: *const Options) -> *const c_char;

    /// Sets the environment.
    #[link_name = "sentry_options_set_environment"]
    pub fn options_set_environment(opts: *mut Options, environment: *const c_char);

    /// Gets the environment.
    #[link_name = "sentry_options_get_environment"]
    pub fn options_get_environment(opts: *const Options) -> *const c_char;

    /// Sets the dist.
    #[link_name = "sentry_options_set_dist"]
    pub fn options_set_dist(opts: *mut Options, dist: *const c_char);

    /// Gets the dist.
    #[link_name = "sentry_options_get_dist"]
    pub fn options_get_dist(opts: *const Options) -> *const c_char;

    /// Configures the http proxy.
    #[link_name = "sentry_options_set_http_proxy"]
    pub fn options_set_http_proxy(opts: *mut Options, proxy: *const c_char);

    /// Returns the configured http proxy.
    #[link_name = "sentry_options_get_http_proxy"]
    pub fn options_get_http_proxy(opts: *const Options) -> *const c_char;

    /// Configures the path to a file containing ssl certificates for
    /// verification.
    #[link_name = "sentry_options_set_ca_certs"]
    pub fn options_set_ca_certs(opts: *mut Options, path: *const c_char);

    /// Returns the configured path for ca certificates.
    #[link_name = "sentry_options_get_ca_certs"]
    pub fn options_get_ca_certs(opts: *const Options) -> *const c_char;

    /// Enables or disables debug printing mode.
    #[link_name = "sentry_options_set_debug"]
    pub fn options_set_debug(opts: *mut Options, debug: c_int);

    /// Returns the current value of the debug flag.
    #[link_name = "sentry_options_get_debug"]
    pub fn options_get_debug(opts: *const Options) -> c_int;

    /// Sets the sentry-native logger function.
    /// Used for logging debug events when the `debug` option is set to true.
    #[link_name = "sentry_options_set_logger"]
    pub fn options_set_logger(opts: *mut Options, logger_func: Option<LoggerFunction>);

    /// Enables or disabled user consent requirements for uploads.
    ///
    /// This disables uploads until the user has given the consent to the SDK.
    /// Consent itself is given with `sentry_user_consent_give` and
    /// `sentry_user_consent_revoke`.
    #[link_name = "sentry_options_set_require_user_consent"]
    pub fn options_set_require_user_consent(opts: *mut Options, val: c_int);

    /// Returns true if user consent is required.
    #[link_name = "sentry_options_get_require_user_consent"]
    pub fn options_get_require_user_consent(opts: *const Options) -> c_int;

    /// Enables or disables on-device symbolication of stack traces.
    ///
    /// This feature can have a performance impact, and is enabled by default on
    /// Android. It is usually only needed when it is not possible to provide
    /// debug information files for system libraries which are needed for
    /// serverside symbolication.
    #[link_name = "sentry_options_set_symbolize_stacktraces"]
    pub fn options_set_symbolize_stacktraces(opts: *const Options, val: c_int);

    /// Returns true if on-device symbolication of stack traces is enabled.
    #[link_name = "sentry_options_get_symbolize_stacktraces"]
    pub fn options_get_symbolize_stacktraces(opts: *const Options) -> c_int;

    /// Adds a new attachment to be sent along.
    ///
    /// `path` is assumed to be in platform-specific filesystem path encoding.
    /// API Users on windows are encouraged to use
    /// `sentry_options_add_attachmentw` instead.
    #[link_name = "sentry_options_add_attachment"]
    pub fn options_add_attachment(opts: *mut Options, name: *const c_char, path: *const c_char);

    /// Sets the path to the crashpad handler if the crashpad backend is used.
    ///
    /// The path defaults to the `crashpad_handler`/`crashpad_handler.exe`
    /// executable, depending on platform, which is expected to be present in
    /// the same directory as the app executable.
    ///
    /// It is recommended that library users set an explicit handler path,
    /// depending on the directory/executable structure of their app.
    ///
    /// `path` is assumed to be in platform-specific filesystem path encoding.
    /// API Users on windows are encouraged to use
    /// `sentry_options_set_handler_pathw` instead.
    #[link_name = "sentry_options_set_handler_path"]
    pub fn options_set_handler_path(opts: *mut Options, path: *const c_char);

    /// Sets the path to the Sentry Database Directory.
    ///
    /// Sentry will use this path to persist user consent, sessions, and other
    /// artifacts in case of a crash. This will also be used by the crashpad
    /// backend if it is configured.
    ///
    /// The path defaults to `.sentry-native` in the current working directory,
    /// will be created if it does not exist, and will be resolved to an
    /// absolute path inside of `sentry_init`.
    ///
    /// It is recommended that library users set an explicit absolute path,
    /// depending on their apps runtime directory.
    ///
    /// `path` is assumed to be in platform-specific filesystem path encoding.
    /// API Users on windows are encouraged to use
    /// `sentry_options_set_database_pathw` instead.
    #[link_name = "sentry_options_set_database_path"]
    pub fn options_set_database_path(opts: *mut Options, path: *const c_char);

    /// Wide char version of `sentry_options_add_attachment`.
    #[link_name = "sentry_options_add_attachmentw"]
    pub fn options_add_attachmentw(opts: *mut Options, name: *const c_char, path: *const c_wchar);

    /// Wide char version of `sentry_options_set_handler_path`.
    #[link_name = "sentry_options_set_handler_pathw"]
    pub fn options_set_handler_pathw(opts: *mut Options, path: *const c_wchar);

    /// Wide char version of `sentry_options_set_database_path`
    #[link_name = "sentry_options_set_database_pathw"]
    pub fn options_set_database_pathw(opts: *mut Options, path: *const c_wchar);

    /// Enables forwarding to the system crash reporter. Disabled by default.
    ///
    /// This setting only has an effect when using Crashpad on macOS. If
    /// enabled, Crashpad forwards crashes to the macOS system crash
    /// reporter. Depending on the crash, this may impact the crash time.
    /// Even if enabled, Crashpad may choose not to forward certain crashes.
    #[link_name = "sentry_options_set_system_crash_reporter_enabled"]
    pub fn options_set_system_crash_reporter_enabled(opts: *mut Options, enabled: c_int);

    /// Initializes the Sentry SDK with the specified options.
    ///
    /// This takes ownership of the options.  After the options have been set
    /// they cannot be modified any more.
    #[link_name = "sentry_init"]
    pub fn init(options: *mut Options) -> c_int;

    /// Shuts down the Sentry client and forces transports to flush out.
    #[link_name = "sentry_shutdown"]
    pub fn shutdown();

    /// Clears the internal module cache.
    ///
    /// For performance reasons, Sentry will cache the list of loaded libraries
    /// when capturing events. This cache can get out-of-date when loading
    /// or unloading libraries at runtime. It is therefore recommended to
    /// call `sentry_clear_modulecache` when doing so, to make sure that the
    /// next call to `sentry_capture_event` will have an up-to-date module
    /// list.
    #[link_name = "sentry_clear_modulecache"]
    pub fn clear_modulecache();

    /// Returns the client options.
    ///
    /// This might return NULL if Sentry is not yet initialized.
    #[link_name = "sentry_get_options"]
    pub fn get_options() -> *const Options;

    /// Gives user consent.
    #[link_name = "sentry_user_consent_give"]
    pub fn user_consent_give();

    /// Revokes user consent.
    #[link_name = "sentry_user_consent_revoke"]
    pub fn user_consent_revoke();

    /// Resets the user consent (back to unknown).
    #[link_name = "sentry_user_consent_reset"]
    pub fn user_consent_reset();

    /// Checks the current state of user consent.
    #[link_name = "sentry_user_consent_get"]
    pub fn user_consent_get() -> UserConsent;

    /// Sends a Sentry event.
    #[link_name = "sentry_capture_event"]
    pub fn capture_event(event: Value) -> Uuid;

    /// Adds the breadcrumb to be sent in case of an event.
    #[link_name = "sentry_add_breadcrumb"]
    pub fn add_breadcrumb(breadcrumb: Value);

    /// Sets the specified user.
    #[link_name = "sentry_set_user"]
    pub fn set_user(user: Value);

    /// Removes a user.
    #[link_name = "sentry_remove_user"]
    pub fn remove_user();

    /// Sets a tag.
    #[link_name = "sentry_set_tag"]
    pub fn set_tag(key: *const c_char, value: *const c_char);

    /// Removes the tag with the specified key.
    #[link_name = "sentry_remove_tag"]
    pub fn remove_tag(key: *const c_char);

    /// Sets extra information.
    #[link_name = "sentry_set_extra"]
    pub fn set_extra(key: *const c_char, value: Value);

    /// Removes the extra with the specified key.
    #[link_name = "sentry_remove_extra"]
    pub fn remove_extra(key: *const c_char);

    /// Sets a context object.
    #[link_name = "sentry_set_context"]
    pub fn set_context(key: *const c_char, value: Value);

    /// Removes the context object with the specified key.
    #[link_name = "sentry_remove_context"]
    pub fn remove_context(key: *const c_char);

    /// Sets the event fingerprint.
    ///
    /// This accepts a variable number of arguments, and needs to be terminated
    /// by a trailing `NULL`.
    #[link_name = "sentry_set_fingerprint"]
    pub fn set_fingerprint(fingerprint: *const c_char, ...);

    /// Removes the fingerprint.
    #[link_name = "sentry_remove_fingerprint"]
    pub fn remove_fingerprint();

    /// Sets the transaction.
    #[link_name = "sentry_set_transaction"]
    pub fn set_transaction(transaction: *const c_char);

    /// Removes the transaction.
    #[link_name = "sentry_remove_transaction"]
    pub fn remove_transaction();

    /// Sets the event level.
    #[link_name = "sentry_set_level"]
    pub fn set_level(level: i32);

    /// Starts a new session.
    #[link_name = "sentry_start_session"]
    pub fn start_session();

    /// Ends a session.
    #[link_name = "sentry_end_session"]
    pub fn end_session();
}
