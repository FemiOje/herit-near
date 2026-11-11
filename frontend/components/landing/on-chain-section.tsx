import { Reveal } from "@/components/ui/reveal";
import { ON_CHAIN } from "@/lib/content/landing";
import { accountUrl } from "@/lib/near/config";

/** What's deployed, linked to NearBlocks, so anyone can check it. */
export function OnChainSection() {
  return (
    <section id="contracts" className="relative scroll-mt-24 overflow-hidden px-4 py-14 sm:px-6 sm:py-20">
      <div className="relative z-10 mx-auto max-w-6xl">
        <Reveal>
          <span className="tag tag-shadow bg-pink">under the hood</span>
          <h2 className="mt-5 max-w-2xl">live on NEAR testnet</h2>
          <p className="mt-4 max-w-xl text-base text-muted">
            one factory, and one keyless account per estate running code nobody can change. the Selfie Check is verified with World
            and signed for the estate by an attestor: on testnet a server route; an OutLayer TEE worker is built for mainnet, so no
            server holds the key.
          </p>
        </Reveal>

        <div className="mt-10 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {ON_CHAIN.map((c, index) => (
            <Reveal key={c.name} delay={index * 55}>
              <article className="card-flat flex h-full flex-col p-5">
                <h3 className="text-[0.95rem]">{c.name}</h3>
                <p className="mt-2.5 flex-1 text-sm leading-relaxed text-muted">{c.body}</p>
                {c.detail ? <p className="mono mt-3 break-all text-[0.7rem] text-muted">hash {c.detail}</p> : null}
                <div className="mt-4 border-t-2 border-ink/10 pt-3">
                  <a href={accountUrl(c.account)} target="_blank" rel="noopener noreferrer" className="mono inline-flex items-center gap-1 break-all text-[0.75rem] font-bold underline decoration-2 underline-offset-4">
                    {c.account}
                    <span aria-hidden="true">↗</span>
                  </a>
                </div>
              </article>
            </Reveal>
          ))}
        </div>
      </div>
    </section>
  );
}
