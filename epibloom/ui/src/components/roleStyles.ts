export interface RoleStyle {
    label: string;
    labelClass: string;
    borderClass: string;
    highlightBorderClass: string;
}

export const ROLE_STYLES: Record<string, RoleStyle> = {
    system: {
        label: "System",
        labelClass: "text-neutral-400",
        borderClass: "border-neutral-800",
        highlightBorderClass: "border-neutral-500",
    },
    user: {
        label: "User",
        labelClass: "text-blue-400",
        borderClass: "border-blue-900",
        highlightBorderClass: "border-blue-600",
    },
    assistant: {
        label: "Assistant",
        labelClass: "text-green-400",
        borderClass: "border-green-900",
        highlightBorderClass: "border-green-600",
    },
};

export function roleStyle(role: string): RoleStyle {
    return ROLE_STYLES[role] ?? ROLE_STYLES.system;
}
