import type { PromotionSeedSuite } from "./synthetic";

interface Seed {
  formula: string;
  lawId: string;
  refusals: readonly string[];
  roles: Readonly<Record<string, string>>;
}

function law(seed: Seed): PromotionSeedSuite["laws"][number] {
  const refusalCategories = [
    "wrong-operator",
    "role-mismatch",
    "missing-role-evidence",
    "wrong-sign",
    "extra-term",
  ] as const;
  const separator = seed.formula.indexOf("=");
  if (separator < 1 || separator === seed.formula.length - 1) {
    throw new Error(seed.lawId + ": positive seed must be an equality");
  }
  const left = seed.formula.slice(0, separator);
  const right = seed.formula.slice(separator + 1);
  const positiveFormulas = [
    seed.formula,
    left + " = " + right,
    "{" + left + "}={" + right + "}",
    right + "=" + left,
    "\\displaystyle " + left + "=" + right,
  ];
  return {
    lawId: seed.lawId,
    positives: positiveFormulas.map((formula, index) => [
      seed.lawId + "-positive-" + (index + 1),
      "The reviewed formula uses locally declared linear-algebra roles. $" +
        formula +
        "$",
      formula,
      seed.roles,
    ]),
    refusals: seed.refusals.map((formula, index) => [
      seed.lawId + "-refusal-" + (index + 1),
      "The altered expression has no declarations that establish the target roles. $" +
        formula +
        "$",
      formula,
      refusalCategories[index % refusalCategories.length]!,
    ]),
  };
}

const seeds: readonly Seed[] = [
  {
    lawId: "matrix-addition",
    formula: "C=A+B",
    roles: { result: "C", left: "A", right: "B" },
    refusals: ["C=AB", "C=A-B", "C=A/B", "C=A", "A+B+C"],
  },
  {
    lawId: "matrix-inverse-definition",
    formula: "B=\\operatorname{inv}(A)",
    roles: { inverse: "B", operator: "A" },
    refusals: ["B=A^T", "B=A", "B=A^{-2}", "B=1/A", "A^{-1}+B"],
  },
  {
    lawId: "determinant-definition",
    formula: "d=\\det(A)",
    roles: { result: "d", operator: "A" },
    refusals: [
      "d=\\operatorname{tr}(A)",
      "d=A",
      "d=\\det(A)+1",
      "\\det(A)=\\det(B)",
      "d=\\det(A^T)",
    ],
  },
  {
    lawId: "trace-definition",
    formula: "t=\\operatorname{tr}(A)",
    roles: { result: "t", operator: "A" },
    refusals: [
      "t=\\det(A)",
      "t=A",
      "t=\\operatorname{tr}(A)+1",
      "\\operatorname{tr}(A)=\\operatorname{tr}(B)",
      "t=\\operatorname{tr}(A^T)",
    ],
  },
  {
    lawId: "matrix-adjoint-definition",
    formula: "B=\\operatorname{adj}(A)",
    roles: { result: "B", operator: "A" },
    refusals: ["B=A^T", "B=A^{-1}", "B=A", "B=A^2", "A^*+B"],
  },
  {
    lawId: "inner-product-definition",
    formula: "s=\\langle x,y\\rangle",
    roles: { result: "s", left: "x", right: "y" },
    refusals: [
      "s=x+y",
      "s=xy",
      "s=\\lVert x\\rVert",
      "s=\\langle x,y\\rangle+1",
      "\\langle x,y\\rangle",
    ],
  },
  {
    lawId: "vector-norm-definition",
    formula: "r=\\lVert x\\rVert",
    roles: { result: "r", vector: "x" },
    refusals: [
      "r=x",
      "r=x^2",
      "r=\\langle x,y\\rangle",
      "r=\\lVert x\\rVert+1",
      "\\lVert x\\rVert",
    ],
  },
  {
    lawId: "vector-orthogonality",
    formula: "\\langle x,y\\rangle=0",
    roles: { left: "x", right: "y" },
    refusals: [
      "\\langle x,y\\rangle=1",
      "\\langle x,y\\rangle>0",
      "x+y=0",
      "xy=0",
      "\\langle x,y\\rangle=q",
    ],
  },
  {
    lawId: "eigenpair-equation",
    formula: "Av=\\lambda v",
    roles: { operator: "A", vector: "v", value: "lambda" },
    refusals: [
      "Av=\\lambda w",
      "Av=w",
      "A+v=\\lambda v",
      "vA=\\lambda v",
      "Av=0",
    ],
  },
  {
    lawId: "matrix-diagonalization",
    formula: "AP=PD",
    roles: { operator: "A", basis: "P", diagonal: "D" },
    refusals: ["A=PD", "A=DP^{-1}", "A=PDP", "A=P^{-1}DP", "A=P+D+P^{-1}"],
  },
  {
    lawId: "symmetric-matrix-definition",
    formula: "A=A^T",
    roles: { operator: "A" },
    refusals: ["A=B^T", "A=A^{-1}", "A=A^H", "A^T=B", "A=A"],
  },
  {
    lawId: "hermitian-matrix-definition",
    formula: "A=\\operatorname{adj}(A)",
    roles: { operator: "A" },
    refusals: ["A=B^*", "A=A^{-1}", "A=A^T", "A^*=B", "A=A"],
  },
  {
    lawId: "positive-definite-quadratic-form",
    formula: "q=\\operatorname{quad}(A,x)",
    roles: { result: "q", operator: "A", vector: "x" },
    refusals: ["x^TAx=0", "x^TAy>0", "x^TAx<0", "Ax>0", "x^Tx>0"],
  },
  {
    lawId: "lu-factorization",
    formula: "A=LU",
    roles: { operator: "A", lower: "L", upper: "U" },
    refusals: ["A=UL", "A=L+U", "A=L", "A=L^{-1}U", "LU+B"],
  },
  {
    lawId: "qr-factorization",
    formula: "A=QR",
    roles: { operator: "A", orthogonal: "Q", upper: "R" },
    refusals: ["A=RQ", "A=Q+R", "A=Q", "A=Q^{-1}R", "QR+B"],
  },
  {
    lawId: "cholesky-factorization",
    formula: "A=LL^T",
    roles: { operator: "A", lower: "L" },
    refusals: ["A=L^TL", "A=LR^T", "A=LL", "A=L+L^T", "LL^T+B"],
  },
  {
    lawId: "orthogonal-eigendecomposition",
    formula: "A=Q\\Lambda Q^T",
    roles: { operator: "A", basis: "Q", values: "Lambda" },
    refusals: [
      "A=Q\\Lambda Q^{-1}",
      "A=Q\\Lambda",
      "A=\\Lambda QQ^T",
      "A=Q\\Lambda R^T",
      "Q\\Lambda Q^T+B",
    ],
  },
  {
    lawId: "singular-value-decomposition",
    formula: "A=U\\Sigma V^T",
    roles: { operator: "A", left: "U", values: "Sigma", right: "V" },
    refusals: [
      "A=U\\Sigma U^T",
      "A=U\\Sigma V^{-1}",
      "A=U\\Sigma",
      "A=\\Sigma UV^T",
      "U\\Sigma V^T+B",
    ],
  },
  {
    lawId: "pseudoinverse-solution",
    formula: "x=Ab",
    roles: { solution: "x", inverse: "A", observation: "b" },
    refusals: ["x=A+b", "x=A^{-1}b", "x=bA", "x=A", "Ab+y"],
  },
  {
    lawId: "rank-nullity-theorem",
    formula: "n=r+k",
    roles: { dimension: "n", rank: "r", nullity: "k" },
    refusals: ["n=r-k", "n=rk", "n=r", "n=r+k+1", "r+k"],
  },
  {
    lawId: "basis-expansion",
    formula: "x=\\sum_i c_i v_i",
    roles: { vector: "x", coordinate: "c", basis: "v" },
    refusals: [
      "x=\\sum_i c_i",
      "x=\\sum_i v_i",
      "x=c_iv_j",
      "x=\\prod_i c_iv_i",
      "\\sum_i c_iv_i",
    ],
  },
  {
    lawId: "orthogonal-projection",
    formula: "y=Px",
    roles: { result: "y", projector: "P", vector: "x" },
    refusals: ["y=xP", "y=P+x", "y=P", "y=P^{-1}x", "Px+z"],
  },
];

export const linearAlgebraFoundationSuite: PromotionSeedSuite = {
  id: "linear-algebra-foundation-probe",
  laws: seeds.map(law),
  packId: "linear-algebra",
};
