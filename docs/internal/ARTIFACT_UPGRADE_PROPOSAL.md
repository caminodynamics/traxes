# TRAXES Artifact Evidence Upgrade - Implementation Proposal

## Current Artifact Structure Analysis

### Existing Schema (`src/artifact.rs`)

```rust
pub struct AuditArtifact {
    pub artifact_version: String,              // "1.0.0"
    pub artifact_type: String,                // "pre_execution_decision"
    pub decision_id: String,
    pub timestamp: String,
    pub decision: String,                     // "ALLOW" | "DENY"
    pub tool: String,
    pub environment: String,
    pub reason: String,
    pub policy_bundle: String,                // "infra-cost-limit-v1"
    pub policy_hash: String,                  // SHA256 hash
    pub sha256_hash: String,                 // Artifact integrity
    pub engine: EngineInfo,
    pub execution_context: ExecutionContext,
    pub proposed_action: ProposedActionInfo,
    pub rule_evaluation: RuleEvaluationInfo,  // SINGLE rule only
    pub performance: PerformanceInfo,
    pub side_effect_prevention: SideEffectPreventionInfo,
    pub execution_status: String,
}
```

### Current Gaps
- No explicit policy version (only hash)
- No governance coverage status
- No replay verification status in artifact
- Single rule evaluation only
- No evaluation trace for debugging

---

## Proposed Changes

### 1. Add Policy Version Field

**Current Structure:**
```rust
pub struct EngineInfo {
    pub name: String,
    pub engine_version: String,
    pub policy_bundle_id: String,  // Only ID, no version
}
```

**Proposed Structure:**
```rust
pub struct EngineInfo {
    pub name: String,
    pub engine_version: String,
    pub policy_bundle_id: String,
    pub policy_version: String,  // NEW: Explicit version field
}

// Alternative: Separate PolicyInfo struct
pub struct PolicyInfo {
    pub policy_id: String,
    pub policy_version: String,
    pub policy_hash: String,
}
```

**Implementation Details:**
- Add `policy_version` field to `EngineInfo` (preferred for minimal change)
- Default value: "1.0.0" for current policies
- Update `AuditArtifact::new()` to populate the field
- Maintain backward compatibility by making field optional or providing default
- Update SHA256 hash calculation to include policy version

**Impact:**
- **Breaking Change:** Yes (new field in struct)
- **Backward Compatibility:** Can be maintained with `Option<String>` or default value
- **Migration:** Existing v1 artifacts can be loaded with default version

---

### 2. Add Governance Coverage Status

**Proposed New Struct:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceInfo {
    pub coverage_status: String,  // "GOVERNED" | "UNGOVERNED" | "PARTIALLY_GOVERNED"
    pub endpoint: String,
    pub enforcement_hit: bool,
    pub coverage_event_type: Option<String>,  // Reference to coverage module
}
```

**Integration into AuditArtifact:**
```rust
pub struct AuditArtifact {
    // ... existing fields ...
    pub governance: GovernanceInfo,  // NEW
    // ... rest of fields ...
}
```

**Implementation Details:**
- Create new `GovernanceInfo` struct
- Add field to `AuditArtifact`
- Integrate with existing `coverage` module (already implemented)
- Default to "UNGOVERNED" if coverage tracking not enabled
- Update artifact generation to call coverage tracker

**Impact:**
- **Breaking Change:** Yes (new field in struct)
- **Backward Compatibility:** Can be maintained with `Option<GovernanceInfo>`
- **Integration:** Requires hooking into existing coverage module

---

### 3. Add Replay Verification Status

**Proposed New Struct:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayInfo {
    pub replayable: bool,
    pub verification_status: String,  // "VERIFIED" | "FAILED" | "NOT_ATTEMPTED"
    pub last_replay_result: String,  // "MATCH" | "MISMATCH" | "UNKNOWN"
    pub last_replay_timestamp: Option<String>,
    pub replay_count: u32,
}
```

**Integration into AuditArtifact:**
```rust
pub struct AuditArtifact {
    // ... existing fields ...
    pub replay: ReplayInfo,  // NEW
    // ... rest of fields ...
}
```

**Implementation Details:**
- Create new `ReplayInfo` struct
- Initialize with `verification_status: "NOT_ATTEMPTED"` on artifact creation
- Update replay logic to write back to artifact after verification
- Requires artifact update capability (currently read-only in replay)
- Default values: `replayable: true`, `verification_status: "NOT_ATTEMPTED"`

**Impact:**
- **Breaking Change:** Yes (new field in struct)
- **Backward Compatibility:** Can be maintained with `Option<ReplayInfo>`
- **Complexity:** Requires artifact update/write capability in replay module

---

### 4. Improve Multi-Rule Evaluation Support

**Current Structure:**
```rust
pub struct AuditArtifact {
    pub rule_evaluation: RuleEvaluationInfo,  // SINGLE rule
}
```

**Proposed Structure:**
```rust
pub struct AuditArtifact {
    pub rules_evaluated: Vec<RuleEvaluationInfo>,  // MULTIPLE rules
    // OR maintain backward compatibility:
    pub rule_evaluation: RuleEvaluationInfo,  // DEPRECATED but kept
    pub rules_evaluated: Option<Vec<RuleEvaluationInfo>>,  // NEW
}
```

**Enhanced RuleEvaluationInfo:**
```rust
pub struct RuleEvaluationInfo {
    pub rule_id: String,
    pub field: String,
    pub observed_value: serde_json::Value,
    pub operator: String,
    pub policy_value: f64,
    pub evaluation_expression: String,
    pub evaluation_result: bool,
    pub rule_order: u32,  // NEW: For multi-rule ordering
    pub rule_severity: Option<String>,  // NEW: "HIGH" | "MEDIUM" | "LOW"
}
```

**Implementation Details:**
- Change `rule_evaluation` to `rules_evaluated: Vec<RuleEvaluationInfo>`
- Update policy evaluation to collect all rule results
- Add `rule_order` field for evaluation sequence
- Maintain backward compatibility by keeping single field as deprecated
- Update replay logic to handle multiple rules

**Impact:**
- **Breaking Change:** Yes (field type change)
- **Backward Compatibility:** Requires dual-field approach or migration
- **Complexity:** Requires changes to policy evaluation logic

---

### 5. Add Evaluation Trace

**Proposed New Struct:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationTrace {
    pub steps: Vec<EvaluationStep>,
    pub total_evaluation_time_us: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationStep {
    pub step_name: String,           // "policy_loaded" | "rule_evaluated" | "decision_produced"
    pub step_order: u32,
    pub step_result: String,         // "SUCCESS" | "FAILURE"
    pub step_duration_us: f64,
    pub step_details: Option<String>, // Optional context
}
```

**Integration into AuditArtifact:**
```rust
pub struct AuditArtifact {
    // ... existing fields ...
    pub evaluation_trace: Option<EvaluationTrace>,  // NEW
    // ... rest of fields ...
}
```

**Implementation Details:**
- Create `EvaluationTrace` and `EvaluationStep` structs
- Make field optional (not all evaluations need detailed traces)
- Add tracing hooks in policy evaluation flow
- Record: policy load, each rule evaluation, final decision
- Update SHA256 hash to include trace (if present)

**Impact:**
- **Breaking Change:** No (optional field)
- **Backward Compatibility:** Full (optional field)
- **Performance:** Minimal overhead when disabled

---

## Proposed Artifact Version Update

**Current:** `artifact_version: "1.0.0"`  
**Proposed:** `artifact_version: "2.0.0"`

**Rationale:**
- Multiple new fields added
- Structural changes to rule evaluation
- New governance and replay sections
- Breaking changes requiring migration path

---

## Backward Compatibility Strategy

### Option 1: Dual-Version Support (Recommended)
- Keep v1 struct as `AuditArtifactV1`
- Create v2 struct as `AuditArtifactV2`
- Add migration function: `AuditArtifactV1::to_v2()`
- Use version field to determine deserialization target

### Option 2: Optional Fields with Defaults
- Make all new fields `Option<T>`
- Provide default values in `Default` impl
- Simpler but less explicit about version differences

### Option 3: Breaking Change with Migration Script
- Update struct directly
- Provide migration utility for existing artifacts
- Cleanest but requires explicit migration step

**Recommendation:** Option 1 (Dual-Version Support) for clarity and safety.

---

## Implementation Order

1. **Policy Version** (Low risk, high value)
   - Add field to `EngineInfo`
   - Update artifact generation
   - Add tests

2. **Evaluation Trace** (No breaking change)
   - Add optional trace struct
   - Add tracing hooks
   - Add tests

3. **Governance Coverage** (Medium risk)
   - Add `GovernanceInfo` struct
   - Integrate with coverage module
   - Add tests

4. **Multi-Rule Support** (High complexity)
   - Change to Vec<RuleEvaluationInfo>
   - Update policy evaluation
   - Update replay logic
   - Add tests

5. **Replay Status** (Requires artifact write capability)
   - Add `ReplayInfo` struct
   - Add artifact update logic
   - Add tests

---

## Test Requirements

### Unit Tests
- [ ] Policy version serialization/deserialization
- [ ] Governance coverage field population
- [ ] Replay status field initialization
- [ ] Multi-rule evaluation ordering
- [ ] Evaluation trace step recording
- [ ] Backward compatibility (v1 → v2 migration)

### Integration Tests
- [ ] End-to-end artifact generation with all new fields
- [ ] Replay verification with updated artifact
- [ ] Artifact integrity with new hash calculation
- [ ] Coverage module integration
- [ ] Multi-rule policy evaluation

---

## Questions for Confirmation

1. **Policy Version:** Should we use `EngineInfo.policy_version` or create separate `PolicyInfo` struct?

2. **Backward Compatibility:** Do you prefer dual-version support (v1/v2 structs) or optional fields with defaults?

3. **Multi-Rule Evaluation:** Should we completely replace single rule field, or keep both for compatibility?

4. **Replay Status:** Should replay verification update the original artifact file, or store results separately?

5. **Evaluation Trace:** Should this be enabled by default or require a feature flag?

6. **Implementation Order:** Do you agree with the proposed order, or should we prioritize differently?

---

## Acceptance Criteria Verification

After implementation, a reviewer should be able to answer:

1. **What decision happened?** ✅ (existing `decision` field)
2. **Why did it happen?** ✅ (existing `reason` + enhanced `rules_evaluated`)
3. **What policy version was used?** ✅ (new `policy_version` field)
4. **Was the decision governed?** ✅ (new `governance.coverage_status` field)
5. **Does replay reproduce the original decision?** ✅ (new `replay` field + existing replay logic)

---

## Risk Assessment

**Low Risk:**
- Policy version field addition
- Evaluation trace (optional field)

**Medium Risk:**
- Governance coverage integration
- Replay status tracking

**High Risk:**
- Multi-rule evaluation support (requires policy evaluation changes)
- Backward compatibility migration

---

## Estimated Effort

- Policy Version: 2 hours
- Evaluation Trace: 3 hours
- Governance Coverage: 4 hours
- Multi-Rule Support: 6 hours
- Replay Status: 4 hours
- Tests & Documentation: 4 hours

**Total:** ~23 hours

---

## Next Steps

Await user confirmation on:
1. Proposed structural changes
2. Backward compatibility strategy
3. Implementation order
4. Any modifications to the proposal

Once confirmed, proceed with implementation following the agreed approach.
