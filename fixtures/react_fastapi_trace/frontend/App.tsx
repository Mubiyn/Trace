export async function placeCall(): Promise<void> {
  await fetch("/api/calls", { method: "POST" });
}

export function PlaceCallButton() {
  return <button onClick={placeCall}>Place Call</button>;
}
