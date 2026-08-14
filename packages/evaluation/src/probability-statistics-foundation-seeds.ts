import type { PromotionSeedSuite } from "./synthetic";

interface Seed {
  alternate: {
    formula: string;
    roles: Readonly<Record<string, string>>;
  };
  directional?: boolean;
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
  const positives = [
    { formula: seed.formula, roles: seed.roles },
    { formula: left + " = " + right, roles: seed.roles },
    { formula: "{" + left + "}={" + right + "}", roles: seed.roles },
    {
      formula: seed.directional ? "{" + left + "}=" + right : right + "=" + left,
      roles: seed.roles,
    },
    seed.alternate,
  ];
  return {
    lawId: seed.lawId,
    positives: positives.map(({ formula, roles }, index) => [
      seed.lawId + "-positive-" + (index + 1),
      "The reviewed formula uses locally declared probability-statistics roles. $" +
        formula +
        "$",
      formula,
      roles,
    ]),
    refusals: seed.refusals.map((formula, index) => [
      seed.lawId + "-refusal-" + (index + 1),
      "The altered expression does not establish the reviewed probability-statistics relation. $" +
        formula +
        "$",
      formula,
      refusalCategories[index % refusalCategories.length]!,
    ]),
  };
}

const seeds: readonly Seed[] = [
  {
    alternate: { formula: "m=E(Y)", roles: { mean: "m", variable: "Y" } },
    formula: "\\mu=E(X)",
    lawId: "expected-value-definition",
    roles: { mean: "mu", variable: "X" },
    refusals: ["\\mu=Var(X)", "\\mu=E(Y)", "\\mu=X", "-\\mu=E(X)", "\\mu=E(X)+r"],
  },
  {
    alternate: { formula: "w=\\operatorname{Var}(Y)", roles: { variance: "w", variable: "Y" } },
    formula: "v=\\operatorname{Var}(X)",
    lawId: "variance-value-definition",
    roles: { variance: "v", variable: "X" },
    refusals: ["v=E(X)", "v=\\operatorname{Var}(Y)", "v=X^2", "-v=\\operatorname{Var}(X)", "v=\\operatorname{Var}(X)+r"],
  },
  {
    alternate: {
      formula: "d=\\operatorname{Cov}(U,V)",
      roles: { covariance: "d", "left-variable": "U", "right-variable": "V" },
    },
    formula: "c=\\operatorname{Cov}(X,Y)",
    lawId: "covariance-value-definition",
    roles: { covariance: "c", "left-variable": "X", "right-variable": "Y" },
    refusals: ["c=\\operatorname{Var}(X)", "c=\\operatorname{Cov}(X,Z)", "c=\\operatorname{Cov}(X)", "-c=\\operatorname{Cov}(X,Y)", "c=\\operatorname{Cov}(X,Y)+r"],
  },
  {
    alternate: {
      formula: "P(C\\cap D)=P(C)P(D)",
      roles: { left: "C", right: "D" },
    },
    formula: "P(A\\cap B)=P(A)P(B)",
    lawId: "independent-event-factorization",
    roles: { left: "A", right: "B" },
    refusals: [
      "P(A\\cup B)=P(A)P(B)",
      "P(A\\cap C)=P(A)P(B)",
      "P(A\\cap B)=P(A)+P(B)",
      "P(A\\cap B)=-P(A)P(B)",
      "P(A\\cap B)=P(A)P(B)+r",
    ],
  },
  {
    alternate: {
      formula: "\\int_c^d g(y)\\,dy=1",
      roles: { density: "g", variable: "y", lower: "c", upper: "d" },
    },
    formula: "\\int_a^b f(x)\\,dx=1",
    lawId: "density-normalization",
    roles: { density: "f", variable: "x", lower: "a", upper: "b" },
    refusals: [
      "\\int_a^b f(x)\\,dx=0",
      "\\int_a^b g(x)\\,dx=1",
      "\\int_a^b f(y)\\,dx=1",
      "-\\int_a^b f(x)\\,dx=1",
      "\\int_a^b f(x)\\,dx=1+r",
    ],
  },
  {
    alternate: { formula: "\\sum_i r_i=1", roles: { mass: "r" } },
    formula: "\\sum_i p_i=1",
    lawId: "mass-normalization",
    roles: { mass: "p" },
    refusals: ["\\prod_i p_i=1", "\\sum_i q_i=1", "\\sum_i p_i=0", "-\\sum_i p_i=1", "\\sum_i p_i=1+r"],
  },
  {
    alternate: {
      formula: "G(y)=\\int_c^y g(s)\\,ds",
      roles: { cdf: "G", density: "g", value: "y", variable: "s", lower: "c" },
    },
    formula: "F(x)=\\int_a^x f(t)\\,dt",
    lawId: "cdf-from-density",
    roles: { cdf: "F", density: "f", value: "x", variable: "t", lower: "a" },
    refusals: [
      "F(x)=f(x)",
      "G(x)=\\int_a^x f(t)\\,dt",
      "F(x)=\\int_a^x g(t)\\,dt",
      "F(x)=-\\int_a^x f(t)\\,dt",
      "F(x)=\\int_a^x f(t)\\,dt+r",
    ],
  },
  {
    alternate: { formula: "M=\\prod_i r_i", roles: { likelihood: "M", mass: "r" } },
    formula: "L=\\prod_i p_i",
    lawId: "likelihood-product",
    roles: { likelihood: "L", mass: "p" },
    refusals: ["L=\\sum_i p_i", "L=\\prod_i q_i", "L=p_i", "-L=\\prod_i p_i", "L=\\prod_i p_i+r"],
  },
  {
    alternate: { formula: "j=\\log(M)", roles: { "log-likelihood": "j", likelihood: "M" } },
    formula: "h=\\log(L)",
    lawId: "log-likelihood-definition",
    roles: { "log-likelihood": "h", likelihood: "L" },
    refusals: ["h=\\exp(L)", "h=\\log(M)", "h=L", "-h=\\log(L)", "h=\\log(L)+r"],
  },
  {
    alternate: {
      formula: "h=\\frac{1}{k}\\sum_i y_i",
      roles: { "sample-mean": "h", "sample-size": "k", observation: "y" },
    },
    formula: "m=\\frac{1}{n}\\sum_i x_i",
    lawId: "sample-mean-definition",
    roles: { "sample-mean": "m", "sample-size": "n", observation: "x" },
    refusals: [
      "m=\\sum_i x_i",
      "m=\\frac{1}{k}\\sum_i x_i",
      "m=\\frac{1}{n}\\prod_i x_i",
      "-m=\\frac{1}{n}\\sum_i x_i",
      "m=\\frac{1}{n}\\sum_i x_i+r",
    ],
  },
  {
    alternate: {
      formula: "w=\\frac{1}{e}\\sum_i h_i^2",
      roles: { "sample-variance": "w", "degrees-of-freedom": "e", residual: "h" },
    },
    formula: "v=\\frac{1}{d}\\sum_i r_i^2",
    lawId: "sample-variance-definition",
    roles: { "sample-variance": "v", "degrees-of-freedom": "d", residual: "r" },
    refusals: [
      "v=\\frac{1}{d}\\sum_i r_i",
      "v=\\frac{1}{e}\\sum_i r_i^2",
      "v=\\frac{1}{d}\\prod_i r_i^2",
      "-v=\\frac{1}{d}\\sum_i r_i^2",
      "v=\\frac{1}{d}\\sum_i r_i^2+q",
    ],
  },
  {
    alternate: {
      formula: "q=\\frac{t}{m^{1/2}}",
      roles: { "standard-error": "q", "standard-deviation": "t", "sample-size": "m" },
    },
    formula: "e=\\frac{s}{n^{1/2}}",
    lawId: "standard-error-of-mean",
    roles: { "standard-error": "e", "standard-deviation": "s", "sample-size": "n" },
    refusals: ["e=s/n", "e=t/n^{1/2}", "e=s/n^2", "e=-s/n^{1/2}", "e=s/n^{1/2}+r"],
  },
  {
    alternate: {
      formula: "b=q+kt",
      roles: { "upper-bound": "b", estimate: "q", "critical-value": "k", "standard-error": "t" },
    },
    formula: "u=m+ze",
    lawId: "confidence-upper-bound",
    roles: { "upper-bound": "u", estimate: "m", "critical-value": "z", "standard-error": "e" },
    refusals: ["u=m-ze", "u=q+ze", "u=m+z", "u=-m+ze", "u=m+ze+r"],
  },
  {
    alternate: {
      formula: "C=E(ww^T)",
      roles: { "covariance-matrix": "C", "centered-vector": "w" },
    },
    formula: "\\Sigma=E(zz^T)",
    lawId: "covariance-matrix-definition",
    roles: { "covariance-matrix": "Sigma", "centered-vector": "z" },
    refusals: ["\\Sigma=E(z)", "\\Sigma=E(ww^T)", "\\Sigma=E(z^Tz)", "-\\Sigma=E(zz^T)", "\\Sigma=E(zz^T)+R"],
  },
  {
    alternate: {
      formula: "r=Zc+d",
      roles: { response: "r", design: "Z", parameter: "c", error: "d" },
    },
    formula: "y=Xb+e",
    lawId: "linear-regression-model",
    roles: { response: "y", design: "X", parameter: "b", error: "e" },
    refusals: ["y=X+b+e", "y=Zb+e", "y=Xc+e", "y=Xb-e", "y=Xb+e+r"],
  },
  {
    alternate: {
      formula: "z=By+v",
      roles: { "next-state": "z", transition: "B", state: "y", noise: "v" },
    },
    formula: "x_1=Ax_0+w",
    lawId: "stochastic-state-transition",
    roles: { "next-state": "x_1", transition: "A", state: "x_0", noise: "w" },
    refusals: ["x_1=x_0A+w", "x_1=Bx_0+w", "x_1=A+x_0+w", "x_1=Ax_0-w", "x_1=Ax_0+w+r"],
  },
  {
    alternate: {
      formula: "d=\\operatorname{Cov}(u,v)",
      roles: { autocovariance: "d", "left-state": "u", "right-state": "v" },
    },
    formula: "c=\\operatorname{Cov}(x,y)",
    lawId: "process-autocovariance-definition",
    roles: { autocovariance: "c", "left-state": "x", "right-state": "y" },
    refusals: ["c=\\operatorname{Var}(x)", "c=\\operatorname{Cov}(x,z)", "c=\\operatorname{Cov}(x)", "-c=\\operatorname{Cov}(x,y)", "c=\\operatorname{Cov}(x,y)+r"],
  },
];

export const probabilityStatisticsFoundationSuite: PromotionSeedSuite = {
  id: "probability-statistics-foundation-probe",
  laws: seeds.map(law),
  packId: "probability",
};
