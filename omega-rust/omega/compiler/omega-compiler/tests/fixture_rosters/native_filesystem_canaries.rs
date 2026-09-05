//! Source cases executed by the macOS native filesystem integration target.
//! Samples and generated inline programs are not corpus fixture declarations.

#[derive(Clone, Copy)]
pub(crate) struct Fixture {
    // Only execution uses the short name for existing tags and diagnostics.
    #[allow(dead_code, reason = "corpus inventory reads only the source path")]
    pub(crate) name: &'static str,
    pub(crate) path: &'static str,
}

macro_rules! fixtures {
    ($($constant:ident => ($group:literal, $name:literal)),+ $(,)?) => {
        $(pub(crate) const $constant: Fixture = Fixture {
            name: $name,
            path: concat!($group, "/", $name),
        };)+

        // The dedicated target uses the named constants directly.
        #[allow(dead_code, reason = "inventory entrypoint shared with the dedicated test target")]
        pub(crate) const PASS_CANARIES: &[Fixture] = &[$($constant),+];
    };
}

fixtures! {
    NATIVE_CLOSE => ("filesystem", "native_close"),
    NATIVE_STAT => ("filesystem", "native_stat"),
    NATIVE_CRUD => ("filesystem", "native_crud"),
    NATIVE_DIRS => ("filesystem", "native_dirs"),
    NATIVE_READ_DIR_ITER => ("filesystem", "native_read_dir_iter"),
    NATIVE_FLOCK => ("filesystem", "native_flock"),
    WRAPPER_LOCK_METADATA_EXIT => ("filesystem", "wrapper_lock_metadata_exit"),
    WRAPPER_TIMES_OWNER_LSTAT_EXIT => ("filesystem", "wrapper_times_owner_lstat_exit"),
    DIRFD_REREAD_EXIT => ("filesystem", "dirfd_reread_exit"),
    DIR_WALK_WRAPPERS_EXIT => ("filesystem", "dir_walk_wrappers_exit"),
    NATIVE_AT_OPS => ("filesystem", "native_at_ops"),
    NATIVE_AT_RUNTIME_NAME => ("filesystem", "native_at_runtime_name"),
    NATIVE_APPEND => ("filesystem", "native_append"),
    NATIVE_OPEN_RW => ("filesystem", "native_open_rw"),
    NATIVE_OPEN_CREATE => ("filesystem", "native_open_create"),
    NATIVE_SEEK => ("filesystem", "native_seek"),
    NATIVE_POSITIONED_IO => ("filesystem", "native_positioned_io"),
    NATIVE_ERRNO => ("filesystem", "native_errno"),
    NATIVE_FS_WORKFLOW => ("filesystem", "native_fs_workflow"),
    NATIVE_VALUE_CALL_LITERAL => ("filesystem", "native_value_call_literal"),
    NATIVE_VALUE_CALL_PATH => ("filesystem", "native_value_call_path"),
    NATIVE_VALUE_CALL_LOCAL => ("filesystem", "native_value_call_local"),
    NATIVE_BUFFER_COPY => ("filesystem", "native_buffer_copy"),
    NATIVE_SUBSLICE_COPY => ("filesystem", "native_subslice_copy"),
    NATIVE_COPY_PRESERVE => ("filesystem", "native_copy_preserve"),
    NATIVE_FORWARDED_SLICE_LITERAL => ("filesystem", "native_forwarded_slice_literal"),
    NATIVE_RENAME => ("filesystem", "native_rename"),
    NATIVE_HARD_LINK => ("filesystem", "native_hard_link"),
    NATIVE_SYMLINK => ("filesystem", "native_symlink"),
    NATIVE_SET_LEN => ("filesystem", "native_set_len"),
    NATIVE_PERMISSIONS => ("filesystem", "native_permissions"),
    NATIVE_FCHMOD => ("filesystem", "native_fchmod"),
    NATIVE_CHOWN => ("filesystem", "native_chown"),
    NATIVE_EXISTS => ("filesystem", "native_exists"),
    NATIVE_TRY_EXISTS => ("filesystem", "native_try_exists"),
    NATIVE_FILETYPE => ("filesystem", "native_filetype"),
    NATIVE_CANONICALIZE => ("filesystem", "native_canonicalize"),
    NATIVE_TRY_CLONE => ("filesystem", "native_try_clone"),
    NATIVE_READ_DIR => ("filesystem", "native_read_dir"),
    NATIVE_SYNC => ("filesystem", "native_sync"),
    NATIVE_SYNC_DATA => ("filesystem", "native_sync_data"),
    NATIVE_SET_TIMES => ("filesystem", "native_set_times"),
    NATIVE_FSTAT => ("filesystem", "native_fstat"),
    NATIVE_SYMLINK_METADATA => ("filesystem", "native_symlink_metadata"),
    NATIVE_METADATA_NLINK => ("filesystem", "native_metadata_nlink"),
    NATIVE_METADATA_INO => ("filesystem", "native_metadata_ino"),
    NATIVE_METADATA_CTIME_DEV => ("filesystem", "native_metadata_ctime_dev"),
    NATIVE_METADATA_BLOCKS => ("filesystem", "native_metadata_blocks"),
    NATIVE_METADATA_MODIFIED => ("filesystem", "native_metadata_modified"),
    NATIVE_METADATA_TIMES => ("filesystem", "native_metadata_times"),
    NATIVE_METADATA_READONLY => ("filesystem", "native_metadata_readonly"),
    NATIVE_VALUE_CALL_LET_CHAIN => ("filesystem", "native_value_call_let_chain"),
    NATIVE_WRAPPER_WRITE_ALL => ("filesystem", "native_wrapper_write_all"),
    NATIVE_WRAPPER_EXISTS => ("filesystem", "native_wrapper_exists"),
    NATIVE_VALUE_CALL_GUARD => ("filesystem", "native_value_call_guard"),
    NATIVE_ENUM_RESULT => ("filesystem", "native_enum_result"),
    NATIVE_WRAPPER_WRITE_ALL_RESULT => ("filesystem", "native_wrapper_write_all_result"),
    NATIVE_WRAPPER_TRY_EXISTS => ("filesystem", "native_wrapper_try_exists"),
    NATIVE_WRAPPER_METADATA => ("filesystem", "native_wrapper_metadata"),
    NATIVE_FLOAT_ARG => ("float", "native_float_arg"),
    NATIVE_FLOAT_RETURN => ("float", "native_float_return"),
    NATIVE_FLOAT_TWO_ARGS => ("float", "native_float_two_args"),
    FOREIGN_CONTROL_STATE_RESTORE => ("float", "foreign_control_state_restore"),
    OBJC_GET_CLASS => ("objc", "objc_get_class"),
    OBJC_ALLOC => ("objc", "objc_alloc"),
    OBJC_MSGSEND_SCALAR => ("objc", "objc_msgsend_scalar"),
    FRAMEWORK_CLASSES => ("objc", "framework_classes"),
    NSSTRING_LENGTH => ("objc", "nsstring_length"),
    CGRECT_HFA => ("objc", "cgrect_hfa"),
    NSWINDOW_INIT => ("objc", "nswindow_init"),
    CGIMAGE_BLIT => ("objc", "cgimage_blit"),
    PRESENT_FRAME => ("objc", "present_frame"),
    EVENT_PUMP => ("objc", "event_pump"),
    GUI_BACKEND_VALUECALL => ("objc", "gui_backend_valuecall"),
    NATIVE_GUI_LOOP => ("objc", "native_gui_loop"),
    GUI_IMPL_THROUGH_FIELD => ("objc", "gui_impl_through_field"),
    GUI_WINDOW_I32_ARGS => ("objc", "gui_window_i32_args"),
    MACOS_GUI_MODULE => ("objc", "macos_gui_module"),
    CLOCK_SLEEP => ("objc", "clock_sleep"),
    GUI_PROVIDER_SUBSTITUTION => ("objc", "gui_provider_substitution"),
    INPUT_PROVIDER_SUBSTITUTION => ("objc", "input_provider_substitution"),
    SATURATING_DIVIDE_NATIVE => ("arithmetic", "saturating_divide_native"),
}
