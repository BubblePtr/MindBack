import { Button as BaseButton } from "@base-ui/react/button";
import type { ComponentProps } from "react";
import { cn } from "./utils";

type ButtonVariant = "primary" | "secondary" | "plain";

type ButtonProps = Omit<ComponentProps<typeof BaseButton>, "className"> & {
  className?: string;
  variant?: ButtonVariant;
};

export function Button({
  className,
  focusableWhenDisabled = true,
  type = "button",
  variant = "primary",
  ...props
}: ButtonProps) {
  return (
    <BaseButton
      className={
        variant === "plain"
          ? className
          : cn("ui-button", `ui-button-${variant}`, className)
      }
      focusableWhenDisabled={focusableWhenDisabled}
      type={type}
      {...props}
    />
  );
}
