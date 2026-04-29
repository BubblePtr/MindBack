import { Select as BaseSelect } from "@base-ui/react/select";
import { cn } from "./utils";

type SelectOption = {
  label: string;
  value: string;
};

type SelectProps = {
  ariaLabel?: string;
  className?: string;
  onValueChange: (value: string) => void;
  options: SelectOption[];
  value: string;
};

export function Select({
  ariaLabel,
  className,
  onValueChange,
  options,
  value,
}: SelectProps) {
  return (
    <BaseSelect.Root
      items={options}
      modal={false}
      onValueChange={(nextValue) => {
        if (typeof nextValue === "string") {
          onValueChange(nextValue);
        }
      }}
      value={value}
    >
      <BaseSelect.Trigger
        aria-label={ariaLabel}
        className={cn("ui-select-trigger", className)}
      >
        <BaseSelect.Value />
        <BaseSelect.Icon className="ui-select-icon">v</BaseSelect.Icon>
      </BaseSelect.Trigger>
      <BaseSelect.Portal>
        <BaseSelect.Positioner className="ui-select-positioner" sideOffset={6}>
          <BaseSelect.Popup className="ui-select-popup">
            <BaseSelect.List className="ui-select-list">
              {options.map((option) => (
                <BaseSelect.Item
                  className="ui-select-item"
                  key={option.value}
                  value={option.value}
                >
                  <BaseSelect.ItemText>{option.label}</BaseSelect.ItemText>
                </BaseSelect.Item>
              ))}
            </BaseSelect.List>
          </BaseSelect.Popup>
        </BaseSelect.Positioner>
      </BaseSelect.Portal>
    </BaseSelect.Root>
  );
}
