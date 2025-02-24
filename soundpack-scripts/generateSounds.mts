/// <reference types="bun-types" />
import { resolve } from "path";
function promiseAllWithLimit<T>(n: number, list: (() => Promise<T>)[]): Promise<T[]> {
  const tail = list.splice(n);
  const head = list;
  const resolved: Promise<T>[] = [];
  let processed = 0;

  return new Promise<T[]>((resolve) => {
    for (const x of head) {
      const res = x();
      resolved.push(res);
      res.then((y) => {
        runNext();
        return y;
      });
    }

    function runNext(): void {
      if (processed === tail.length) {
        resolve(Promise.all(resolved));
      } else {
        resolved.push(
          tail[processed]().then((x) => {
            runNext();
            return x;
          }),
        );
        processed++;
      }
    }
  });
}

type SoundPackPhrases = {
  /** User started session let's do this! */
  session_start: string[];
  /** User ended the session. Pat yourself on the back for a great job */
  session_ended: string[];
  /** You completed a todo item! Congrats! */
  todo_complete: string[];
  /** Break time is soon, get ready to wind down current work */
  break_approaching: string[];
  /** Break time is here, take a deep breath, walk around, stretch */
  break_start: string[];
  /** Break is ending, return to work soon */
  break_end_approaching: string[];
  /** Return to work */
  work_resumed: string[];
  /** Work has been going on for a while, take a break */
  break_nudge: string[];
};

export async function generateSounds(options: {
  trialName: string;
  variants: SoundPackPhrases;
  generator: (speechContent: string) => Promise<{ mp3Blob: Blob; pennies: number }>;
}) {
  let totalPennies = 0;
  async function generateSound(soundName: keyof SoundPackPhrases, variantIndex: number, speechContent: string) {
    const speechKey = speechContent
      .replace(/[^a-zA-Z0-9]+/g, "_")
      .toLowerCase()
      .slice(0, 20);
    const outputName = `${trialName}/${soundName}.${variantIndex}-${speechKey}-${trialName}.mp3`;
    const fullPath = resolve(Bun.fileURLToPath(import.meta.url), "../", outputName);

    if (await Bun.file(fullPath).exists()) {
      console.error(`${fullPath} already exists`);
      return;
    }

    const { mp3Blob, pennies } = await options.generator(speechContent);
    totalPennies += pennies;
    // write mp3 to file
    await Bun.write(fullPath, mp3Blob, { createPath: true });

    console.log(fullPath);
  }
  const { trialName, variants } = options;

  await promiseAllWithLimit(
    10,
    Object.entries(variants).flatMap(([soundName, variantList]) =>
      variantList.map(
        (speechContent, variantIndex) => () =>
          generateSound(soundName as keyof SoundPackPhrases, variantIndex, speechContent),
      ),
    ),
  );
  console.log(`Total pennies: ${totalPennies}`);
}
