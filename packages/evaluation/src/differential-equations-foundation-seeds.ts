import type { PromotionSeedSuite } from "./synthetic";

interface Seed {
  alternate: {
    formula: string;
    roles: Readonly<Record<string, string>>;
  };
  alternatePresentation?: {
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
  const positiveCases = [
    { formula: seed.formula, roles: seed.roles },
    seed.alternatePresentation ?? {
      formula: "{" + left + "}={" + right + "}",
      roles: seed.roles,
    },
    {
      formula: seed.directional ? "{" + left + "}=" + right : right + "=" + left,
      roles: seed.roles,
    },
    { formula: "\\displaystyle " + left + "=" + right, roles: seed.roles },
    seed.alternate,
  ];
  return {
    lawId: seed.lawId,
    positives: positiveCases.map(({ formula, roles }, index) => [
      seed.lawId + "-positive-" + (index + 1),
      "The reviewed formula uses locally declared differential-equation roles. $" +
        formula +
        "$",
      formula,
      roles,
    ]),
    refusals: seed.refusals.map((formula, index) => [
      seed.lawId + "-refusal-" + (index + 1),
      "The altered expression does not establish the reviewed differential-equation relation. $" +
        formula +
        "$",
      formula,
      refusalCategories[index % refusalCategories.length]!,
    ]),
  };
}

const seeds: readonly Seed[] = [
  {
    alternate: {
      formula: "\\frac{d z}{d s}=g",
      roles: { state: "z", variable: "s", source: "g" },
    },
    lawId: "first-order-ode-model",
    formula: "\\frac{d y}{d t}=f",
    roles: { state: "y", variable: "t", source: "f" },
    refusals: [
      "y=f",
      "\\frac{d y}{d t}=g",
      "\\frac{d^2 y}{d t^2}=f",
      "\\frac{d y}{d x}=f",
      "\\frac{d y}{d t}=f+r",
    ],
  },
  {
    alternate: {
      formula: "\\frac{d^2 z}{d s^2}=g",
      roles: { state: "z", variable: "s", source: "g" },
    },
    lawId: "second-order-ode-model",
    formula: "\\frac{d^2 y}{d t^2}=f",
    roles: { state: "y", variable: "t", source: "f" },
    refusals: [
      "\\frac{d y}{d t}=f",
      "\\frac{d^2 z}{d t^2}=f",
      "\\frac{d^3 y}{d t^3}=f",
      "\\frac{d^2 y}{d x^2}=f",
      "\\frac{d^2 y}{d t^2}=f+r",
    ],
  },
  {
    alternate: {
      formula: "\\frac{d z}{d s}+c z=d",
      roles: { state: "z", variable: "s", coefficient: "c", source: "d" },
    },
    lawId: "linear-first-order-ode",
    formula: "\\frac{d y}{d t}+a y=b",
    roles: { state: "y", variable: "t", coefficient: "a", source: "b" },
    refusals: [
      "\\frac{d y}{d t}+a=b",
      "\\frac{d y}{d t}+a z=b",
      "\\frac{d y}{d t}-a y=b",
      "\\frac{d y}{d t}+a y=0",
      "\\frac{d y}{d t}+a y=b+r",
    ],
  },
  {
    alternate: {
      formula: "\\frac{d z}{d s}=Bz+c",
      roles: { state: "z", variable: "s", operator: "B", source: "c" },
    },
    lawId: "linear-ode-system",
    formula: "\\frac{d x}{d t}=Ax+b",
    roles: { state: "x", variable: "t", operator: "A", source: "b" },
    refusals: [
      "\\frac{d x}{d t}=xA+b",
      "\\frac{d z}{d t}=Ax+b",
      "\\frac{d x}{d t}=A+x+b",
      "\\frac{d x}{d t}=Ax-b",
      "\\frac{d x}{d t}=Ax+b+r",
    ],
  },
  {
    alternate: {
      formula: "\\frac{\\partial v}{\\partial s}=D\\nabla^2v",
      roles: { field: "v", variable: "s", diffusivity: "D" },
    },
    alternatePresentation: {
      formula: "\\frac{\\partial w}{\\partial r}=K\\nabla^2w",
      roles: { field: "w", variable: "r", diffusivity: "K" },
    },
    lawId: "diffusion-equation",
    formula: "\\frac{\\partial u}{\\partial t}=\\kappa\\nabla^2u",
    roles: { field: "u", variable: "t", diffusivity: "kappa" },
    refusals: [
      "\\frac{\\partial u}{\\partial t}=\\kappa u",
      "\\frac{\\partial v}{\\partial t}=\\kappa\\nabla^2u",
      "\\frac{\\partial u}{\\partial t}=-\\kappa\\nabla^2u",
      "\\frac{\\partial u}{\\partial x}=\\kappa\\nabla^2u",
      "\\frac{\\partial u}{\\partial t}=\\kappa\\nabla^2u+r",
    ],
  },
  {
    alternate: {
      formula: "\\nabla^2v=g",
      roles: { field: "v", source: "g" },
    },
    lawId: "poisson-equation",
    formula: "\\nabla^2u=f",
    roles: { field: "u", source: "f" },
    refusals: [
      "\\nabla u=f",
      "\\nabla^2v=f",
      "\\nabla^2u=-f",
      "\\nabla^2u=0",
      "\\nabla^2u=f+r",
    ],
  },
  {
    alternate: {
      formula: "\\nabla^2v=0",
      roles: { field: "v" },
    },
    lawId: "laplace-equation",
    formula: "\\nabla^2u=0",
    roles: { field: "u" },
    refusals: [
      "\\nabla u=0",
      "\\nabla^2v=0",
      "\\nabla^2u=1",
      "\\nabla^2u=f",
      "\\nabla^2u=r",
    ],
  },
  {
    alternate: {
      formula: "\\frac{\\partial v}{\\partial s}+\\operatorname{div}(G)=0",
      roles: { field: "v", variable: "s", flux: "G" },
    },
    alternatePresentation: {
      formula: "\\frac{\\partial w}{\\partial r}+\\operatorname{div}(H)=0",
      roles: { field: "w", variable: "r", flux: "H" },
    },
    lawId: "conservation-form-equation",
    formula: "\\frac{\\partial u}{\\partial t}+\\operatorname{div}(F)=0",
    roles: { field: "u", variable: "t", flux: "F" },
    refusals: [
      "\\frac{\\partial u}{\\partial t}+F=0",
      "\\frac{\\partial v}{\\partial t}+\\operatorname{div}(F)=0",
      "\\frac{\\partial u}{\\partial t}-\\operatorname{div}(F)=0",
      "\\frac{\\partial u}{\\partial x}+\\operatorname{div}(F)=0",
      "\\frac{\\partial u}{\\partial t}+\\operatorname{div}(F)=r",
    ],
  },
  {
    alternate: {
      formula: "z(s_0)=z_0",
      roles: { state: "z", "initial-time": "s_0", "initial-value": "z_0" },
    },
    lawId: "initial-value-condition",
    formula: "y(t_0)=y_0",
    roles: { state: "y", "initial-time": "t_0", "initial-value": "y_0" },
    refusals: ["y=y_0", "z(t_0)=y_0", "y(t_1)=y_0", "y(t_0)=-y_0", "y(t_0)=y_0+r"],
  },
  {
    alternate: {
      formula: "v(x_c)=h",
      roles: { field: "v", boundary: "x_c", value: "h" },
    },
    lawId: "dirichlet-boundary-condition",
    formula: "u(x_b)=g",
    roles: { field: "u", boundary: "x_b", value: "g" },
    refusals: ["u=g", "v(x_b)=g", "u(x_i)=g", "u(x_b)=-g", "u(x_b)=g+r"],
  },
  {
    alternate: {
      formula: "\\operatorname{normalDerivative}(v,x_c)=h",
      roles: { field: "v", boundary: "x_c", value: "h" },
    },
    lawId: "neumann-boundary-condition",
    formula: "\\operatorname{normalDerivative}(u,x_b)=g",
    roles: { field: "u", boundary: "x_b", value: "g" },
    refusals: [
      "u(x_b)=g",
      "\\operatorname{normalDerivative}(v,x_b)=g",
      "\\operatorname{normalDerivative}(u,x_i)=g",
      "\\operatorname{normalDerivative}(u,x_b)=-g",
      "\\operatorname{normalDerivative}(u,x_b)=g+r",
    ],
  },
  {
    alternate: {
      formula: "m+\\beta v=h",
      roles: { "normal-derivative": "m", coefficient: "beta", field: "v", value: "h" },
    },
    lawId: "robin-boundary-condition",
    formula: "n+\\alpha u=g",
    roles: { "normal-derivative": "n", coefficient: "alpha", field: "u", value: "g" },
    refusals: ["n+u=g", "n+\\alpha v=g", "n-\\alpha u=g", "n+\\alpha u=0", "n+\\alpha u=g+r"],
  },
  {
    alternate: {
      formula: "v_L=v_R",
      roles: { "left-trace": "v_L", "right-trace": "v_R" },
    },
    directional: true,
    lawId: "interface-continuity-condition",
    formula: "u_L=u_R",
    roles: { "left-trace": "u_L", "right-trace": "u_R" },
    refusals: ["u_L=-u_R", "u_L=v_R", "u_L=0", "u_L>u_R", "u_L=u_R+r"],
  },
  {
    alternate: {
      formula: "M(v)=\\mu v",
      roles: { operator: "M", field: "v", eigenvalue: "mu" },
    },
    alternatePresentation: {
      formula: "N(w)=\\rho w",
      roles: { operator: "N", field: "w", eigenvalue: "rho" },
    },
    lawId: "differential-operator-eigenproblem",
    formula: "L(u)=\\lambda u",
    roles: { operator: "L", field: "u", eigenvalue: "lambda" },
    refusals: ["L(u)=u", "L(v)=\\lambda u", "L+u=\\lambda u", "L(u)=-\\lambda u", "L(u)=\\lambda u+r"],
  },
];

export const differentialEquationsFoundationSuite: PromotionSeedSuite = {
  id: "differential-equations-foundation-probe",
  laws: seeds.map(law),
  packId: "calculus-analysis",
};
