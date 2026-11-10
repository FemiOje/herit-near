import Link from "next/link";
import { Footer } from "@/components/layout/footer";
import { SiteNav } from "@/components/layout/site-nav";

/** A placeholder home: Checkpoint 11, Part 4 brings the full landing over from the Sepolia app. */
export default function Home() {
  return (
    <>
      <SiteNav />
      <main className="flex-1 px-4 pb-16 pt-16 sm:px-6">
        <section className="mx-auto max-w-4xl">
          <h1>inheritance gated by proof of life</h1>
          <p className="mt-6 max-w-2xl text-lg text-muted">
            Open a keyless estate on NEAR. Check in with a World ID Selfie Check. If you stop, your heirs&apos;
            soulbound credentials wake up and they claim their share.
          </p>
          <div className="mt-8 flex flex-wrap gap-3">
            <Link href="/dashboard" className="btn">
              go to the app
            </Link>
            <Link href="/claim" className="btn btn-ghost">
              i am an heir
            </Link>
          </div>
        </section>
      </main>
      <Footer />
    </>
  );
}
