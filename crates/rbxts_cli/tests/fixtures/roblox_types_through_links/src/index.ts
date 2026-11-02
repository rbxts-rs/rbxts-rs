// Two roblox-ts errors that hold only when the program knows the types below come from @rbxts/types.
export function isFrame(value: unknown) {
	return value instanceof CFrame;
}

export function copy(position: Vector3) {
	const { ...rest } = position;
	return rest;
}
