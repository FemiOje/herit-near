import { PageHeader } from "@/components/layout/page-header";
import { OpenEstateForm } from "@/components/setup/open-estate-form";

export default function SetupPage() {
  return (
    <div className="mx-auto max-w-6xl space-y-8">
      <PageHeader
        eyebrow="grantor"
        title="open an estate"
        description="One transaction to the Herit factory. Then fund the vault, add your heirs, and do your first Selfie Check."
      />
      <OpenEstateForm />
    </div>
  );
}
