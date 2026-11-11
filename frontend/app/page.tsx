import { CallToAction } from "@/components/landing/call-to-action";
import { ClockSection } from "@/components/landing/clock-section";
import { EstatesExplorer } from "@/components/landing/estates-explorer";
import { Hero } from "@/components/landing/hero";
import { HowItWorksSection } from "@/components/landing/how-it-works-section";
import { OnChainSection } from "@/components/landing/on-chain-section";
import { ProblemSection } from "@/components/landing/problem-section";
import { TracksSection } from "@/components/landing/tracks-section";
import { Footer } from "@/components/layout/footer";
import { SiteNav } from "@/components/layout/site-nav";

export default function LandingPage() {
  return (
    <>
      <SiteNav />
      <main className="flex-1">
        <Hero />
        <ProblemSection />
        <HowItWorksSection />
        <ClockSection />
        {/* After the clock: the statuses in the table only mean something once that section has explained them. */}
        <EstatesExplorer />
        <TracksSection />
        <OnChainSection />
        <CallToAction />
      </main>
      <Footer />
    </>
  );
}
