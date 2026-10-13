// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

package gen

import (
	"crypto/rand"
	"fmt"
	"math/big"
)

// 57896044618658097711785492504343953926418782139537452191302581570759080747168
var secpMaxS = new(big.Int).SetBits([]big.Word{
	16134479119472337056,
	6725966010171805725,
	18446744073709551615,
	9223372036854775807,
})

func makeSecret() (secret []byte) {
	secret = make([]byte, 32)
	if n, err := rand.Read(secret); n != 32 || err != nil {
		panic(fmt.Errorf("error with randomness: %v", err))
	}
	return
}
