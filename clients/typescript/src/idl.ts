import { BorshInstructionCoder, type Idl } from "@anchor-lang/core";
import checkedIdl from "../idl/openpacksduel_escrow.json" with { type: "json" };

const EXPECTED_ADDRESS = "Co198eFfQcmn1WzZRnHV6jxcSLBDCv1qNfPfiBYdCLfS";

function loadCheckedIdl(value: unknown): Idl {
  if (typeof value !== "object" || value === null || !("address" in value)) {
    throw new Error("The checked escrow IDL is malformed");
  }
  if (value.address !== EXPECTED_ADDRESS) {
    throw new Error(
      "The checked escrow IDL does not match the canonical program address",
    );
  }
  return value as Idl;
}

export const ESCROW_IDL = loadCheckedIdl(checkedIdl);
export const instructionCoder = new BorshInstructionCoder(ESCROW_IDL);
