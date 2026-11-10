/** The estate vocabulary the screens render, carried over from the Sepolia app. */
import type { ContractStatus } from "@/lib/near/contracts";

export type EstateStatus = "active" | "grace" | "unlocked";

export function fromContract(status: ContractStatus): EstateStatus {
  return status.toLowerCase() as EstateStatus;
}

/** How each state presents itself: the pill label, the one-line explanation, the swatch. */
export const STATUS_COPY: Record<EstateStatus, { label: string; blurb: string; color: string }> = {
  active: {
    label: "active",
    blurb: "proof of life is current — the estate is sealed",
    color: "bg-teal",
  },
  grace: {
    label: "grace",
    blurb: "a check-in was missed — one selfie check still reverses this",
    color: "bg-yellow",
  },
  unlocked: {
    label: "unlocked",
    blurb: "grace lapsed — the estate belongs to its heirs now",
    color: "bg-coral",
  },
};
