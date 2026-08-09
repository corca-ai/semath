# Semantic Math Analysis Library Proposal

> **구현 상태 — v0.10 (2026-08-08):** recoverable matrix/cases/application IR, include·scope 기반 semantic identity, bounded macro provenance와 재사용 가능한 Worker lifecycle을 추가했다. CorTeX의 LaTeX provider는 wasmtex와 Semath 결과를 한 곳에서 합성하고 interactive parsing은 authoring Worker가 담당한다.

## 1. 개요

Semath는 Markdown/LaTeX 문서에서 수학적 구조와 의미를 복원하는 독립 분석 라이브러리다. 파일 형식이나 수학 DSL을 새로 만들지 않으며, 첫 번째 호스트인 CorTeX에 다음 결과를 제공한다.

* mathematical IR과 symbol graph
* hover, definition, reference와 structural selection
* type, shape, unit, domain constraint
* evidence와 ambiguity가 포함된 diagnostics
* 호스트가 검토하고 적용할 수 있는 edit proposal

분석 core는 처음부터 Rust로 구현하고 browser와 standalone LSP에서 같은 WebAssembly artifact를 사용한다. Rust native build는 test와 benchmark에도 사용한다. CorTeX Worker, wasmtex adapter와 LSP shell은 TypeScript로 둔다.

> **Markdown과 LaTeX는 저장 및 교환 형식으로 유지하고, Semath는 그 위에 호스트가 소비할 수 있는 수학 의미 계층을 제공한다.**

초기 분석 대상은 `.tex`와 `.md` 안의 수학 영역이다. `.bib`는 bibliography 용어를 domain inference의 선택적 evidence로만 사용한다.

### 목표가 아닌 것

* 웹 편집기, PDF preview, 저장소 또는 협업 기능을 만드는 것
* TeX compilation과 package completion을 다시 구현하는 것
* 모든 수식에 하나의 확정적 의미를 강제하는 것
* 낮은 confidence의 추론으로 문서를 자동 수정하는 것
* 초기 버전에서 모든 학문 분야와 자연어를 지원하는 것

---

## 2. 문제와 설계 원칙

LaTeX는 조판 언어이지 형식적인 수학 언어가 아니다. 예를 들어 `p(x)`는 일반 함수, 확률분포 또는 저자가 정의한 연산일 수 있다. 같은 `x`도 section이나 equation에 따라 다른 대상을 뜻할 수 있다.

따라서 Semath는 LaTeX를 하나의 formal semantics로 컴파일하지 않는다. 확실한 구조는 결정적으로 분석하고, 나머지는 claim과 evidence로 표현한다.

핵심 원칙은 다음과 같다.

1. Ambiguity는 예외가 아니라 first-class data다.
2. 모든 semantic result는 원문 source range와 evidence를 보존한다.
3. UI, transport, 저장소와 편집 적용은 호스트가 소유한다.
4. interactive path에서는 bounded analysis만 수행한다.
5. 기능 수보다 false positive와 잘못된 edit 방지를 우선한다.

초기 버전은 LLM을 사용하지 않는다. 장기적으로 LLM을 사용하더라도 결정적 분석으로 남은 ambiguity를 보조하는 역할로 제한한다.

---

## 3. 전체 아키텍처

```text
CorTeX main thread
  ProjectDocumentRegistry
  Monaco composite providers
  edit review / permission policy
             │
             │ project delta / query / cancellation
             ▼
CorTeX AuthoringIntelligenceWorker
  wasmtex language service (TypeScript)
    └─ versioned LaTeX syntax snapshot
         └─ wasmtex → Semath adapter (TypeScript)
              └─ Semath Core (Rust/WASM)
  Markdown frontend → Semath Core

Native clients
  VS Code / Neovim / Zed
             │ LSP
             ▼
  Node/Bun LSP shell
    wasmtex syntax → Semath Core (same WASM artifact)
```

Semath Core는 다음을 모른다.

* Monaco, React와 Yjs
* CorTeX API와 document authority
* wasmtex의 TypeScript 구현 세부
* JSON-RPC, stdio와 Web Worker

CorTeX에서는 JSON-RPC LSP를 의무적으로 거치지 않는다. Worker adapter가 coarse-grained core API를 호출하고 CorTeX가 wasmtex, Markdown, Semath 결과를 합성한다. 외부 편집기에는 같은 core를 사용하는 실제 LSP server를 제공한다.

### Rust/WASM을 처음부터 사용하는 이유

* browser와 standalone LSP가 같은 분석 artifact를 공유한다.
* binder, constraint solver, canonicalization과 e-graph로 확장하기 적합하다.
* TypeScript IR을 나중에 Rust ABI로 옮기는 migration을 피한다.
* deterministic result와 메모리·성능 budget을 명확히 관리할 수 있다.

대신 JS↔WASM serialization, cold start, debugging과 UTF-8↔UTF-16 위치 변환을 초기 설계 문제로 다뤄야 한다. TypeScript shell과 Rust core 사이의 ABI는 작고 versioned되어야 한다.

---

## 4. wasmtex와 Semath의 경계

CorTeX는 이미 wasmtex의 error-tolerant tokenizer, parser, project index, include graph와 package/macro intelligence를 사용한다. Semath가 이 계층을 다시 구현하면 comment, verbatim, unfinished source와 macro 처리 결과가 어긋난다.

소유권은 다음과 같이 나눈다.

```text
wasmtex
  TeX lexical/surface syntax와 error recovery
  comments, verbatim, conditional masking
  math region, macro, include/class/package graph
  TeX package/resource intelligence

Semath
  math region 내부의 mathematical IR
  mathematical binder와 scope
  prose-to-symbol linking과 semantic identity
  type/shape/unit/domain constraint
  formula pattern, canonicalization과 ambiguity
```

현재 검토 기준은 wasmtex의 `latex-tokenizer.ts`, `latex-parser.ts`, `project-index.ts`, `lsp-service.ts`, `lsp-monaco.ts`, `lsp-server.ts`다.

### wasmtex에 필요한 수정

Semath가 private `src/lsp/*`에 의존하지 않도록 `wasmtex/syntax` public entrypoint를 추가한다.

```typescript
interface LatexSyntaxService {
  reset(snapshot: LatexProjectSyntaxInput): void;
  upsert(document: LatexDocumentInput): LatexFileSyntax;
  move(fileId: string, nextPath: string): void;
  remove(fileId: string): void;
  getFile(fileId: string): LatexFileSyntax | null;
}

interface LatexFileSyntax {
  schemaVersion: 2;
  fileId: string;
  path: string;
  documentVersion: number;
  mathRegions: readonly LatexMathRegion[];
  macros: readonly LatexMacroEvent[];
  includes: readonly LatexInclude[];
  diagnostics: readonly LatexSyntaxDiagnostic[];
}
```

이 contract는 다음을 보장한다.

* range는 원문 기준 UTF-16 half-open offset이다.
* unfinished LaTeX에서도 부분 결과를 반환한다.
* macro event는 definition/call range, bounded expansion 상태, cycle/truncation과 synthetic occurrence의 editability를 보존한다.
* stable `fileId`와 mutable path를 구분한다.
* schema version으로 wasmtex와 Semath의 독립 release를 허용한다.

`LatexLanguageService`와 `ProjectIndex`도 동일한 syntax snapshot을 사용하도록 내부 parsing path를 통합한다. 한 번의 update에서 wasmtex index와 Semath 입력을 따로 parse하지 않는 것이 목표다. Worker 안에서는 snapshot을 reference로 전달하고 token stream 전체를 main thread로 복사하지 않는다.

`wasmtex/lsp/monaco`에는 provider별 선택 등록 옵션 또는 개별 provider factory도 필요하다. CorTeX가 겹치는 hover, definition, reference 등을 하나의 composite provider로 합성하기 위해서다.

---

## 5. CorTeX Integration Contract

CorTeX는 다음 authority를 계속 소유한다.

| Concern | Owner |
| --- | --- |
| 활성 project의 text working set와 stable `fileId` | `ProjectDocumentRegistry` |
| editable content와 revision | Yjs / `CollaborationRoom` |
| provider 등록, UI, cancellation | Monaco composition root |
| 권한, review, stale edit 거절, apply/undo | conditional project text edit 흐름 |

현재 주요 통합 지점은 `project-document-registry.ts`, `use-latex-runtime-lsp-bootstrap.ts`, `use-latex-runtime.ts`, `markdown/monaco-intelligence.ts`다.

### CorTeX에 필요한 수정

1. `AuthoringIntelligenceRuntime`을 만들고 registry lifecycle을 Worker에 한 번 전달한다.
2. main thread의 wasmtex editor-query service를 Worker로 옮기고 provider가 async query를 사용하게 한다.
3. Worker 안에서 wasmtex syntax snapshot을 Semath adapter에 직접 전달한다.
4. wasmtex와 Semath가 겹치는 기능은 CorTeX composite provider에서 합성한다.
5. Markdown exclusive provider에 Semath query를 주입한다.
6. 모든 결과에 공통 epoch/version stale-result gate와 cancellation을 적용한다.
7. Semath edit proposal을 기존 review 및 revision-checked edit 흐름에 연결한다.

기존 별도 LaTeX diagnostics Worker는 interactive query latency benchmark가 나오기 전까지 유지할 수 있다. compile Worker와 authoring-intelligence Worker는 항상 분리한다.

Read-only session에서는 탐색 결과만 노출하고 rename, code action과 mutation completion은 제거한다.

---

## 6. Project lifecycle과 WASM ABI

CorTeX registry event를 다음 project protocol로 변환한다.

```typescript
type ProjectChange =
  | { kind: "reset"; snapshot: ProjectSnapshot }
  | { kind: "upsert"; document: ProjectDocument }
  | { kind: "path-change"; fileId: string; path: string }
  | { kind: "remove"; fileId: string };

interface ProjectSnapshot {
  epoch: string;
  inventoryVersion: number;
  projectId: string;
  mainFileId?: string;
  documents: readonly ProjectDocument[];
}

interface ProjectDocument {
  fileId: string;
  path: string;
  language: "latex" | "markdown" | "bibtex";
  content: string;
  documentVersion: number;
  includes?: readonly { path: string; sourceRange: SourceRange }[];
}
```

현재 registry에는 per-document version이 없으므로 adapter가 epoch 안에서 file별 monotonic version을 부여한다. 결과에는 `epoch`, `inventoryVersion`, `fileId`, `documentVersion`, `analysisGeneration`을 포함하며, CorTeX는 현재 상태와 맞지 않는 결과를 폐기한다.

### Coarse-grained ABI

WASM 경계에서 node마다 함수를 호출하지 않는다.

```text
create(config) -> handle
resetProject(handle, encodedSnapshot)
applyChanges(handle, encodedChanges)
query(handle, encodedQuery) -> encodedResult
dispose(handle)
```

payload는 versioned envelope와 `Uint8Array`를 사용한다. 초기 encoding은 구현이 단순한 형식으로 시작하되 JSON, MessagePack 등의 실제 비용을 corpus로 비교한다. ABI 바깥에서는 Rust type과 TypeScript type을 수동으로 중복 정의하지 않고 schema에서 생성하거나 compatibility test로 고정한다.

전체 semantic graph를 update마다 반환하지 않는다. update 결과는 invalidation과 diagnostic summary로 제한하고 hover, equation tree, references는 cursor-driven query로 요청한다. interactive query는 background global inference보다 우선한다.

### Source position과 provenance

Rust 문자열 index는 UTF-8 byte, Monaco/LSP edit range는 UTF-16 code unit 기준이다. document마다 byte↔UTF-16 index를 한 번 만들고 다음 규칙을 지킨다.

* 외부 source range는 UTF-16 half-open offset을 기준으로 한다.
* 내부 parser는 byte range를 사용할 수 있지만 대응하는 source map을 유지한다.
* Markdown에서 추출한 virtual math source는 원문 range로 역매핑할 수 있어야 한다.
* normalization과 macro expansion 이후에도 call-site와 definition provenance를 보존한다.

### Edit proposal

Semath는 문서를 직접 수정하지 않는다.

```typescript
interface SemanticEditProposal {
  title: string;
  safety: "deterministic" | "review-required";
  evidence: readonly Evidence[];
  files: readonly {
    fileId: string;
    documentVersion: number;
    expectedContentHash: string;
    edits: readonly {
      startOffset: number;
      endOffset: number;
      expectedText: string;
      replacementText: string;
    }[];
  }[];
}
```

CorTeX가 최신 collaborative snapshot과 expected text를 재검증하고 review 후 적용한다. `deterministic`은 의미 보존을 증명할 수 있다는 뜻이지 호스트의 권한 및 stale check를 생략한다는 뜻이 아니다.

---

## 7. Processing Pipeline과 IR

```text
Project snapshot / delta
        ↓
wasmtex syntax adapter ─── Markdown math/prose frontend
        └──────────────┬─────────────────────────────┘
                       ↓
          Rust mathematical parser / Structural IR
                       ↓
             Binding and scope analysis
                       ↓
        Explicit definition/prose extraction
                       ↓
            Constraint and evidence graph
                       ↓
       Optional domain pattern recognition
                       ↓
            Document Semantic Graph
```

초기 invalidation 단위는 file이다. dependency edge를 따라 영향받는 file만 재분석하고, generation cancellation으로 오래된 결과를 버린다. 실제 subtree reuse가 구현되기 전에는 “incremental AST”라고 부르지 않는다.

IR은 presentation structure와 mathematical structure를 구분한다.

```text
Presentation: fraction, root, superscript, delimiter, matrix, cases
Mathematical: sum, integral, limit, application, operator, norm, expectation
```

예를 들어 `1/N \sum_{i=1}^N f(x_i)`는 product, fraction, sum, binder `i`와 body로 표현한다. `f`의 의미를 확정하지 않아도 structural selection과 binder analysis에는 충분하다.

Markdown frontend는 fenced/inline code, HTML comment와 frontmatter를 수식으로 해석하지 않는다. 지원하는 math delimiter의 원문 range만 Rust core에 전달한다.

---

## 8. Symbol, scope와 binder

Symbol graph의 key는 glyph 문자열이 아니라 scope에 속한 semantic identity다.

```text
project include component
└── document
    └── section / environment
        └── equation or prose definition region
            └── mathematical binder
```

같은 `x`라도 definition과 scope가 다르면 별도 symbol이다. `x_i`, `x^{(t)}`, `\hat{x}`는 자동으로 같은 symbol이 아니라 related-form candidate다.

명시적인 binder는 가능한 한 결정적으로 처리한다.

```latex
\sum_{i=1}^N x_i
\lim_{n\to\infty} a_n
\forall x\in X
```

Bound variable rename에는 다음 조건이 필요하다.

* binder body와 모든 occurrence의 source range가 완전하다.
* 새 이름이 free variable을 capture하지 않는다.
* macro-expanded occurrence가 안전한 원문 range에 대응한다.
* unfinished expression에서는 rename을 거부한다.

적분의 `dx`, expectation subscript와 quantified prose처럼 문법만으로 확정할 수 없는 표기는 candidate로 두고 evidence를 기록한다.

---

## 9. Evidence와 ambiguity

숫자 confidence 하나를 결론에 직접 붙이지 않고 claim과 evidence를 분리한다.

```typescript
interface SemanticClaim<T> {
  subject: SemanticNodeId;
  value: T;
  status: "certain" | "supported" | "speculative" | "unknown" | "conflicting";
  evidence: readonly Evidence[];
  conflicts: readonly ClaimId[];
}

interface Evidence {
  ruleId: string;
  kind:
    | "syntax"
    | "explicit-math"
    | "explicit-prose"
    | "macro-name"
    | "document-structure"
    | "domain-prior"
    | "derived-constraint";
  strength: "hard" | "strong" | "weak";
  sourceRanges: readonly SourceRange[];
}
```

상태의 의미는 다음과 같다.

| Status | 의미 |
| --- | --- |
| `certain` | syntax 또는 명시적 선언으로 결정됨 |
| `supported` | 복수의 강한 evidence가 일치함 |
| `speculative` | prior나 약한 pattern만 존재함 |
| `unknown` | 후보를 좁힐 evidence가 없음 |
| `conflicting` | 양립할 수 없는 claim이 존재함 |

내부 solver가 weight를 사용하더라도 이를 보편적인 확률처럼 API에 노출하지 않는다. 사용자-facing status는 versioned corpus의 calibration 결과로부터 파생한다. 모든 non-unknown claim은 rule ID와 source evidence를 가져야 한다.

---

## 10. Prose와 domain knowledge

초기 목표는 자연어 이해가 아니라 **명시적인 semantic linking**이다.

```text
Let <MATH> denote <NP>
We denote <NP> by <MATH>
where <MATH> is <NP>
<MATH> \in <MATH>
```

초기 지원 언어는 영어다. 미지원 언어에서도 수식 구조 분석은 그대로 제공한다. 이후 respectively, apposition, parenthetical definition과 notation table을 추가할 수 있다.

Macro 이름과 document structure도 evidence가 될 수 있다. 예를 들어 `\newcommand{\loss}{\mathcal L}`의 `loss`는 유용한 hint지만 확정 의미는 아니다.

Domain pack은 operator, typed formula pattern, constraint rule, notation prior와 vocabulary를 제공한다. 하나의 문서를 하나의 domain으로 강제하지 않고 section이나 equation별로 여러 pack을 활성화할 수 있다. Domain prior는 definition이나 warning을 단독으로 만들지 못한다.

첫 pack은 linear algebra다. Probability/statistics, optimization/ML, physics는 실제 사용자 가치와 corpus가 준비되는 순서로 추가한다.

전통 NLP는 다음 조건을 만족할 때만 도입한다.

* pattern baseline보다 corpus 품질이 높다.
* browser cold start, memory와 latency budget을 만족한다.
* model license와 offline distribution이 명확하다.

---

## 11. Constraint, formula pattern과 rewrite

Constraint system은 type과 refinement를 함께 표현한다.

```text
Scalar, Integer, Vector[d], Matrix[m,n], Tensor[...]
Distribution[X], NormalizedDistribution[X]
SymmetricMatrix[n], PositiveSemiDefiniteMatrix[n]
PhysicalDimension[L/T]
```

명시적 선언과 안전한 propagation을 우선한다.

```text
W : Matrix[m,d]     x : Vector[d]
---------------------------------
Wx : Vector[m]
```

Typed formula pattern은 문자열 snippet이 아니라 parameter, 요구 constraint와 result를 가진 semantic object다. 같은 pattern을 recognition과 completion에 사용한다.

Canonicalization은 alpha renaming, presentation 차이와 조건이 없는 안전한 normalization부터 시작한다. 수학적 identity에는 side condition이 필요하다. 예를 들어 `log(ab) = log a + log b`는 양의 실수 scalar 같은 domain 가정 없이는 rewrite하지 않는다.

Canonicalization, semantic fingerprint와 e-graph saturation은 모든 keystroke의 필수 pipeline이 아니다. 해당 query가 요청될 때 budget을 두고 실행하는 later feature다.

---

## 12. 사용자 기능과 안전 정책

| 기능 | Semath 결과 | Host/LSP 연결 |
| --- | --- | --- |
| Equation tree | bounded structural query | custom inspection UI |
| Structural selection | nested source ranges | `textDocument/selectionRange` |
| Symbol hover | claims, type, evidence | `textDocument/hover` |
| Definition/references | stable symbol locations | 표준 definition/references |
| Rename | validated edit proposal | `prepareRename` / `rename` |
| Diagnostics | evidence-bearing problems | push 또는 pull diagnostics |
| Formula completion | typed text edit proposal | `textDocument/completion` |
| Formula rewrite | side-condition-aware preview proposal | `textDocument/codeAction` |

Diagnostic은 다음 범주를 지원할 수 있다.

* explicit shape/type contradiction
* notation collision within the same scope
* possible use before explicit definition
* defined-but-unused explicit notation
* domain 또는 unit inconsistency

명시적인 contradiction만 warning 후보로 삼는다. Scientific writing convention에 의존하는 항목은 hint 또는 inspection result로 시작하고 false-positive budget을 통과한 rule만 승격한다.

Semantic confidence와 edit safety는 별개다. 예를 들어 `xW`의 shape가 맞지 않아도 `Wx`로 바꾸면 저자의 의도가 달라질 수 있으므로 quick fix를 제공하지 않는다. 안전한 대안이 없으면 원인과 propagation chain만 설명한다.

---

## 13. LSP와 custom protocol

표준 method를 우선 사용한다.

```text
hover, definition, references, prepareRename, rename
documentSymbol, workspace/symbol, codeAction, completion
semanticTokens, inlayHint, selectionRange, diagnostics
```

Scientific-specific query는 versioned `semath/` namespace를 사용한다.

```text
semath/v1/equationTree
semath/v1/symbolInfo
semath/v1/constraints
semath/v1/explainDiagnostic
```

Custom query는 document 또는 node 중심으로 bounded되어야 하며 cancellation과 pagination을 지원한다. 전체 graph를 한 번에 JSON으로 반환하지 않는다.

---

## 14. Repository 구조

```text
crates/
  semath-core/       # parser, IR, graph, constraints, query
  semath-wasm/       # wasm-bindgen ABI
  semath-native/     # native test and benchmark harness

packages/
  wasmtex-adapter/   # wasmtex/syntax → Semath input
  worker/            # browser lifecycle and cancellation
  lsp/               # Node/Bun LSP shell using wasmtex + WASM

packs/
  linear-algebra/
  probability/       # 필요한 iteration에서 추가

fixtures/
  corpus and protocol regression runner
```

Core crate는 wasm-bindgen이나 LSP type에 의존하지 않는다. Browser Worker와 LSP shell은 같은 `semath-wasm` artifact를 사용한다. 순수 Rust LSP binary는 wasmtex와 동등한 native syntax frontend가 생기기 전에는 만들지 않는다. Domain pack의 data와 rule implementation 경계는 첫 linear-algebra pack에서 검증한 뒤 확정한다.

---

## 15. 사용자 가치 중심 로드맵

각 iteration은 architecture layer가 아니라 CorTeX 사용자가 확인할 수 있는 vertical slice다.

### Iteration 1 — 수식 구조 탐색

* AST 단위 structural selection과 equation tree를 제공한다.
* Rust core, WASM ABI, `wasmtex/syntax`, Worker lifecycle과 source mapping을 함께 검증한다.
* malformed Markdown/LaTeX에서도 분석 가능한 부분만 반환한다.

### Iteration 2 — 명시적 정의와 symbol navigation

* `Let x denote ...`, `where W \in ...`, notation table을 definition으로 연결한다.
* hover에 정의 문장, 명시적 type과 evidence를 표시한다.
* 같은 scope의 확실한 references를 찾는다.

### Iteration 3 — Binder-aware rename

* sum, limit, quantifier의 free/bound variable을 구분한다.
* capture-avoiding rename proposal을 CorTeX review 흐름으로 적용한다.
* 적분 differential과 prose binder는 아직 rename하지 않는다.
* **v0.2 완료:** 합과 극한의 scope는 바로 다음 구조 단위로 보수적으로 제한하고, 한정자는 해당 sequence의 나머지를 scope로 삼는다. 불완전한 수식, shadowing 또는 capture 가능성이 있는 변경은 proposal을 만들지 않는다.

### Iteration 4 — 기본 shape insight

* 명시적인 vector/matrix 선언과 propagation을 hover에 표시한다.
* linear-algebra pack으로 확실한 shape contradiction을 설명한다.
* 수정 action 없이 evidence chain만 제공한다.
* **v0.3 완료:** `\mathbb{R}`의 vector/matrix 선언, alias·덧셈·행렬 곱의 shape를 보수적으로 전파한다. 서로 다른 symbolic dimension만으로는 경고하지 않고, 숫자 불일치 또는 명시적 부등식으로 모순이 증명된 경우만 진단한다.

### Iteration 5 — Notation consistency

* 같은 scope의 충돌하는 type/role과 중복 definition을 찾는다.
* used-before-defined와 unused definition은 검증된 경우에만 hint로 제공한다.
* **v0.3 일부 완료:** 같은 문서에서 한 symbol을 양립할 수 없는 shape로 재선언한 경우를 진단한다. use-before-definition과 unused-definition은 false-positive corpus가 준비될 때까지 보류한다.
* **v0.5 role consistency:** 명시적 영어 definition에서 set, function/operator, probability distribution, random variable, index role을 추출한다. 같은 effective scope의 상호 배타적 role 또는 role–shape 선언만 모든 충돌 source와 함께 warning으로 반환한다. Domain prior와 관습 표기는 진단에 사용하지 않고 sibling section의 shadowing은 허용한다.
* **v0.5 definition hygiene:** 단일 문서에서 유일하고 강한 명시적 definition과 완결된 free occurrence가 확인될 때만 used-before-definition과 defined-but-unused를 hint로 제공한다. 다중 문서·중복 definition·notation table·binder·미완성 수식은 보류하며, warning 승격은 별도 라벨 corpus와 측정된 precision 기준을 통과해야 한다.

### Iteration 6 — Formula recognition과 completion

* 먼저 linear algebra formula를 typed pattern으로 인식하고 probability는 검증 corpus를 갖춘 후 별도 pack으로 추가한다.
* 현재 symbol table에 맞춘 semantic completion을 제공한다.
* equivalent-form rewrite는 preview-only로 유지한다.
* **v0.4 완료:** versioned linear-algebra pack의 matrix/vector product, transpose, inner product와 quadratic form을 recognition과 completion이 함께 사용한다. Section scope와 scalar/vector/matrix/tensor constraint를 반영하며, completion은 자동 적용하지 않고 CorTeX의 revision-checked review로 전달한다. Rewrite와 probability pack은 후속 iteration으로 남긴다.
* **v0.6 probability slice:** 명시적으로 event와 random variable로 정의된 단일 기호에 대해 event probability, expectation과 variance를 typed pattern으로 인식한다. Conditional probability는 conditioning event에 positive-probability 근거가 있을 때만 허용한다. 명시적 scalar target의 completion만 review-required proposal로 제공하며 rewrite는 포함하지 않는다.
* **v0.7 rewrite slice:** conditional probability의 conditioning event에 positive-probability 근거가 있을 때 정의식 전개를 제안한다. Bayes 전개는 두 event 모두에 같은 근거가 있을 때만 제안한다. 닫힌 math region에 rewrite target이 하나뿐이면 좌변·delimiter·수식 직후의 cursor에서도 발견할 수 있다. 결과는 bounded query이며 exact expected text를 포함한 review-required proposal로만 반환하고 Quick Fix나 자동 적용은 하지 않는다.

### Iteration 7 — Explainable prose와 domain inference

* 더 넓은 영어 definition pattern과 domain evidence를 연결한다.
* 적용된 prior와 diagnostic evidence chain을 inspection할 수 있다.
* NLP는 corpus와 browser budget을 만족할 때만 포함한다.
* **v0.5 완료:** `symbolInfo`는 한 기호의 정의, 가시적인 shape claim, 인식된 formula, 관련 diagnostic과 source evidence를 각각 최대 8개로 묶어 반환한다. `domainEvidence`는 section/equation 범위의 linear-algebra와 probability pack 활성화 근거를 별도로 반환한다. 어휘·표기 prior는 weak inspection evidence로만 쓰고 definition이나 warning을 만들지 않으며, typed formula match만 해당 equation에서 strong evidence가 된다.
* **v0.5 prose slice:** `respectively`, apposition, parenthetical definition, typed `For each/every`, 문장 단위 `is/represents`만 stable rule로 인식한다. 명시된 scalar/vector/matrix/tensor와 dimension, symmetric·diagonal·positive-definite·normalized refinement는 scoped strong evidence가 되며 기존 수학 선언과 충돌해도 양쪽 claim을 보존한다. 주변 단어만으로는 추론하지 않는다.

### Iteration 8 — Semantic Math Inspector

* 하나의 bounded `inspection` query가 equation tree와 선택 경로, symbol 정보, 정의·참조, 진단, domain/formula evidence와 사용 가능한 edit proposal을 같은 snapshot에서 반환한다.
* CorTeX Math 사이드바는 cursor와 양방향으로 연결하고, source 이동은 즉시 수행하되 completion·rewrite·rename은 기존 revision-checked review와 권한 정책을 그대로 사용한다.
* 큰 수식과 많은 참조는 node/depth/result budget을 적용하고 truncation을 명시한다.

### Iteration 9 — Shared authoring runtime과 standalone LSP

* wasmtex language service와 Semath가 stable file identity를 가진 동일한 syntax snapshot을 소비한다.
* CorTeX의 LaTeX completion, hover, definition, references와 Semath query를 하나의 cancellable Worker queue로 통합하고 cursor query를 background 분석보다 우선한다.
* `semath/lsp`와 `semath-lsp`가 selection, hover, navigation, rename, completion, diagnostics와 review-required code action을 같은 WASM core로 제공한다.
* parse count, cold start, cursor p95와 응답 크기를 release gate로 고정한다.

### Iteration 10 — Reliable project-wide semantics

* matrix/cases row·cell, function application과 paired delimiter를 unfinished source에서도 bounded partial IR로 반환한다.
* include component, stable file identity, section scope와 declaration anchor를 semantic symbol ID로 사용해 같은 glyph의 false link를 막는다.
* wasmtex macro provenance와 CorTeX composite provider를 통해 synthetic occurrence는 탐색만 허용하고 모든 edit는 source-backed review plan으로 제한한다.
* 다중 파일 false-link corpus, native/WASM/LSP parity, Worker failure/cancellation과 cursor latency를 하나의 release gate로 묶는다.

### Iteration 11–13 — 기존 기능의 신뢰도와 상호작용 완성

* 다섯 수학 pack의 schema·corpus·precision gate를 통일하고, cursor 경계와 project scope를 순수 정책으로 고정한다.
* Inspector는 AST 자체보다 의미, 문제, 근거와 가능한 action을 우선한다.
* 브라우저 E2E 조합은 줄이고 corpus, Rust core와 순수 presentation model이 대부분의 경우를 소유한다.

### Iteration 14 — 수학·공학 확장을 위한 과학 의미 기반

* schema-3 pack은 namespace, dependency/capability, concept, law와 role을 명시하며 catalog가 모든 외부 참조를 검증한다.
* domain-neutral semantic graph가 claim, conflict, relation과 evidence를 bounded query로 제공한다.
* quantity kind, unit과 유리수 지수 dimension을 정확히 표현하고 명시적 선언과 곱·나눗셈만 보수적으로 전파한다.
* classical mechanics, circuits와 control systems를 pilot pack으로 추가하되 명시적 quantity/shape가 없으면 전문 law로 인식하지 않는다.
* 기존 다섯 pack은 같은 schema로 이관하되 동작과 edit authority를 넓히지 않는다.

Later work에는 더 넓은 quantity calculus, 분야별 ontology/corpus, semantic fingerprint,
e-graph, cross-paper navigation과 optional LLM assistance가 포함된다. 수백 분야 지원은
core 분기 추가가 아니라 의존성이 명시된 pack과 독립 scorecard의 누적으로 진행한다.

---

## 16. 검증, 성능과 build

각 iteration은 다음 release gate를 갖는다.

* real-world 및 malformed document corpus
* precision, recall, false-link 또는 false-diagnostic budget
* deterministic edit의 false-edit zero corpus
* typing/query p50·p95 latency, memory, cold start와 WASM size budget
* rapid edit, project switch, move/remove의 stale-result test
* native core test와 browser/LSP WASM의 golden-result parity
* syntax, ABI, rule과 pack schema version compatibility test

구체적인 수치는 첫 corpus baseline에서 정하고 이후 임의로 완화하지 않는다.

### 원격 build runner

Release용 Rust/WASM artifact는 Apple Silicon 개발 머신에서 빌드하지 않고, 별도의 원격 빌드 호스트에서 생성한다. 다만 특정 머신의 수동 전역 상태에 의존하지 않도록 다음을 repository에 고정한다.

* `rust-toolchain.toml`, `Cargo.lock`
* 사용하는 wasm-bindgen과 target version
* source sync → test → WASM build → artifact 회수를 수행하는 한 명령
* release host의 clean build, checksum과 artifact provenance
* CI의 독립 build, generated ABI와 native/browser WASM parity test

원격 호스트는 빠른 cache를 가진 release build runner다. Host별 LLVM codegen bytes가 같다고 가정하지 않고, CI는 별도 clean build의 동작 parity와 generated ABI를 검증한다.

---

## 17. 장기 방향

Semath의 장기 가치는 공식 목록 자체보다 다음 요소의 결합에서 나온다.

```text
real-world source recovery
binding-aware symbol resolution
evidence-bearing semantic graph
typed domain patterns and constraints
safe, explainable transformations
browser/native parity
```

Knowledge pack이 축적될수록 navigation, diagnostics, recognition과 completion이 함께 개선된다. 그 과정에서도 plain-text portability와 표준 Markdown/LaTeX 호환성은 유지한다.

> **표현을 편집하는 도구에서 의미를 탐색하고 검증하는 도구로.**
