import { Field as BaseField } from "@base-ui/react/field";
import { Input as BaseInput } from "@base-ui/react/input";
import { forwardRef, type ComponentProps } from "react";
import { cn } from "./utils";

type FieldRootProps = Omit<ComponentProps<typeof BaseField.Root>, "className"> & {
  className?: string;
};
type FieldLabelProps = Omit<ComponentProps<typeof BaseField.Label>, "className"> & {
  className?: string;
};
type InputProps = Omit<ComponentProps<typeof BaseInput>, "className"> & {
  className?: string;
};
type TextareaProps = Omit<ComponentProps<typeof BaseField.Control>, "className" | "render"> & {
  className?: string;
};

function Root({ className, ...props }: FieldRootProps) {
  return <BaseField.Root className={cn("ui-field", className)} {...props} />;
}

function Label({ className, ...props }: FieldLabelProps) {
  return <BaseField.Label className={cn("ui-field-label", className)} {...props} />;
}

const Input = forwardRef<HTMLElement, InputProps>(function Input(
  { className, ...props },
  ref,
) {
  return <BaseInput className={cn("ui-input", className)} ref={ref} {...props} />;
});

const Textarea = forwardRef<HTMLTextAreaElement, TextareaProps>(function Textarea(
  { className, ...props },
  ref,
) {
  return (
    <BaseField.Control
      className={cn("ui-input", "ui-textarea", className)}
      render={<textarea ref={ref} />}
      {...props}
    />
  );
});

export const Field = {
  ...BaseField,
  Root,
  Label,
};

export { Input, Textarea };
