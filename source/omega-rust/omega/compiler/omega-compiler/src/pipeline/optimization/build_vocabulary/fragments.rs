pub(super) const DECLARATIONS: &str = r#"pub data Optimization {
    case ControlFlowCleanup;
    case SparseConditionalConstantPropagation;
    case CopyPropagation;
    case GlobalValueNumbering;
    case DeadPureScalarElimination;
    case ProofCheckElision;
    case SelectedIncomingU12ExactAddImmediate;
    case X86RelaxConditionalBranchesToRel8V1;
    case SelectedIncomingU12ExactSubtractImmediate;
    case Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1;
    case SharedEntryFixedViewCopyAfterCompareBeforeBranchV1;
    case ActiveResidentImmediateU64MultiUseRematerializationV1;
    case Aarch64SelectShortestMovnSeededI64MaterializationV1;
    case X86SelectXorZeroI64MaterializationV1;
    case X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1;
    case X86SelectMovR64Imm32SignExtendedI64MaterializationV1;
    case Aarch64ElideSameViewCopyI64BeforeReturnV1;
    case Aarch64ElideSameViewCopyI64BeforeCompareZeroV1;
    case Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1;
}
pub data Optimizations {
    human_report: u8 in Trapping;
    control_flow_cleanup: u8 in Trapping;
    sparse_conditional_constant_propagation: u8 in Trapping;
    copy_propagation: u8 in Trapping;
    global_value_numbering: u8 in Trapping;
    dead_pure_scalar_elimination: u8 in Trapping;
    proof_check_elision: u8 in Trapping;
    selected_incoming_u12_exact_add_immediate: u8 in Trapping;
    x86_relax_conditional_branches_to_rel8_v1: u8 in Trapping;
    selected_incoming_u12_exact_subtract_immediate: u8 in Trapping;
    aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1: u8 in Trapping;
    shared_entry_fixed_view_copy_after_compare_before_branch_v1: u8 in Trapping;
    active_resident_immediate_u64_multi_use_rematerialization_v1: u8 in Trapping;
    aarch64_select_shortest_movn_seeded_i64_materialization_v1: u8 in Trapping;
    x86_select_xor_zero_i64_materialization_v1: u8 in Trapping;
    x86_select_mov_r32_imm32_zero_extended_i64_materialization_v1: u8 in Trapping;
    x86_select_mov_r64_imm32_sign_extended_i64_materialization_v1: u8 in Trapping;
    aarch64_elide_same_view_copy_i64_before_return_v1: u8 in Trapping;
    aarch64_elide_same_view_copy_i64_before_compare_zero_v1: u8 in Trapping;
    aarch64_elide_same_view_copy_i64_before_compare_i64_left_operand_v1: u8 in Trapping;
}
"#;

pub(super) const ENABLE_MACHINE: &str = r#"pub machine Optimizations::enable(&mut self, optimization: Optimization) {
    transition optimization {
        Optimization::ControlFlowCleanup -> control_flow_cleanup()
        Optimization::SparseConditionalConstantPropagation -> sparse_conditional_constant_propagation()
        Optimization::CopyPropagation -> copy_propagation()
        Optimization::GlobalValueNumbering -> global_value_numbering()
        Optimization::DeadPureScalarElimination -> dead_pure_scalar_elimination()
        Optimization::ProofCheckElision -> proof_check_elision()
        Optimization::SelectedIncomingU12ExactAddImmediate -> selected_incoming_u12_exact_add_immediate()
        Optimization::X86RelaxConditionalBranchesToRel8V1 -> x86_relax_conditional_branches_to_rel8_v1()
        Optimization::SelectedIncomingU12ExactSubtractImmediate -> selected_incoming_u12_exact_subtract_immediate()
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 -> aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1()
        Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1 -> shared_entry_fixed_view_copy_after_compare_before_branch_v1()
        Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1 -> active_resident_immediate_u64_multi_use_rematerialization_v1()
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1 -> aarch64_select_shortest_movn_seeded_i64_materialization_v1()
        Optimization::X86SelectXorZeroI64MaterializationV1 -> x86_select_xor_zero_i64_materialization_v1()
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1 -> x86_select_mov_r32_imm32_zero_extended_i64_materialization_v1()
        Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1 -> x86_select_mov_r64_imm32_sign_extended_i64_materialization_v1()
        Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1 -> aarch64_elide_same_view_copy_i64_before_return_v1()
        Optimization::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1 -> aarch64_elide_same_view_copy_i64_before_compare_zero_v1()
        Optimization::Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1 -> aarch64_elide_same_view_copy_i64_before_compare_i64_left_operand_v1()
    }

    state control_flow_cleanup(&mut self) {
        self.control_flow_cleanup = self.control_flow_cleanup + 1;
    }

    state sparse_conditional_constant_propagation(&mut self) {
        self.sparse_conditional_constant_propagation = self.sparse_conditional_constant_propagation + 1;
    }

    state copy_propagation(&mut self) {
        self.copy_propagation = self.copy_propagation + 1;
    }

    state global_value_numbering(&mut self) {
        self.global_value_numbering = self.global_value_numbering + 1;
    }

    state dead_pure_scalar_elimination(&mut self) {
        self.dead_pure_scalar_elimination = self.dead_pure_scalar_elimination + 1;
    }

    state proof_check_elision(&mut self) {
        self.proof_check_elision = self.proof_check_elision + 1;
    }

    state selected_incoming_u12_exact_add_immediate(&mut self) {
        self.selected_incoming_u12_exact_add_immediate = self.selected_incoming_u12_exact_add_immediate + 1;
    }

    state x86_relax_conditional_branches_to_rel8_v1(&mut self) {
        self.x86_relax_conditional_branches_to_rel8_v1 = self.x86_relax_conditional_branches_to_rel8_v1 + 1;
    }

    state selected_incoming_u12_exact_subtract_immediate(&mut self) {
        self.selected_incoming_u12_exact_subtract_immediate = self.selected_incoming_u12_exact_subtract_immediate + 1;
    }

    state aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1(&mut self) {
        self.aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1 = self.aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1 + 1;
    }

    state shared_entry_fixed_view_copy_after_compare_before_branch_v1(&mut self) {
        self.shared_entry_fixed_view_copy_after_compare_before_branch_v1 = self.shared_entry_fixed_view_copy_after_compare_before_branch_v1 + 1;
    }

    state active_resident_immediate_u64_multi_use_rematerialization_v1(&mut self) {
        self.active_resident_immediate_u64_multi_use_rematerialization_v1 = self.active_resident_immediate_u64_multi_use_rematerialization_v1 + 1;
    }

    state aarch64_select_shortest_movn_seeded_i64_materialization_v1(&mut self) {
        self.aarch64_select_shortest_movn_seeded_i64_materialization_v1 = self.aarch64_select_shortest_movn_seeded_i64_materialization_v1 + 1;
    }

    state x86_select_xor_zero_i64_materialization_v1(&mut self) {
        self.x86_select_xor_zero_i64_materialization_v1 = self.x86_select_xor_zero_i64_materialization_v1 + 1;
    }

    state x86_select_mov_r32_imm32_zero_extended_i64_materialization_v1(&mut self) {
        self.x86_select_mov_r32_imm32_zero_extended_i64_materialization_v1 = self.x86_select_mov_r32_imm32_zero_extended_i64_materialization_v1 + 1;
    }

    state x86_select_mov_r64_imm32_sign_extended_i64_materialization_v1(&mut self) {
        self.x86_select_mov_r64_imm32_sign_extended_i64_materialization_v1 = self.x86_select_mov_r64_imm32_sign_extended_i64_materialization_v1 + 1;
    }

    state aarch64_elide_same_view_copy_i64_before_return_v1(&mut self) {
        self.aarch64_elide_same_view_copy_i64_before_return_v1 = self.aarch64_elide_same_view_copy_i64_before_return_v1 + 1;
    }

    state aarch64_elide_same_view_copy_i64_before_compare_zero_v1(&mut self) {
        self.aarch64_elide_same_view_copy_i64_before_compare_zero_v1 = self.aarch64_elide_same_view_copy_i64_before_compare_zero_v1 + 1;
    }

    state aarch64_elide_same_view_copy_i64_before_compare_i64_left_operand_v1(&mut self) {
        self.aarch64_elide_same_view_copy_i64_before_compare_i64_left_operand_v1 = self.aarch64_elide_same_view_copy_i64_before_compare_i64_left_operand_v1 + 1;
    }
}
"#;

pub(super) const REPORT_MACHINE: &str = r#"pub machine Optimizations::emit_report(&mut self) {
    self.human_report = self.human_report + 1;
}
"#;
