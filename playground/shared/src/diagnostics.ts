export function secondaryAnnotationsWithMessages<
  T extends {
    primary: boolean;
    message: string | null | undefined;
  },
>(annotations: readonly T[]): Array<T & { message: string }> {
  let seenPrimary = false;

  return annotations.filter(
    (annotation): annotation is T & { message: string } => {
      if (!seenPrimary && annotation.primary) {
        seenPrimary = true;
        return false;
      }

      return annotation.message != null && annotation.message.length > 0;
    },
  );
}
