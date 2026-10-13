// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

package websocket

import "math/rand"

func generateSubscriptionId() int {
	return rand.Intn(1000)
}
