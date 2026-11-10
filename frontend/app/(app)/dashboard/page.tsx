import { PageHeader } from "@/components/layout/page-header";
import { MyEstates } from "@/components/dashboard/my-estates";

export default function DashboardPage() {
  return (
    <div className="mx-auto max-w-6xl space-y-8">
      <PageHeader eyebrow="grantor" title="your estates" description="Each estate is its own keyless NEAR account, opened through the Herit factory." />
      <MyEstates />
    </div>
  );
}
