//------------------------------------------------------------------------------------
// runtime.rs -- Part of RHoiScribe
//
// Copyright (C) 2026 CzXieDdan. All rights reserved.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// https://github.com/czxieddan/RHoiScribe
//------------------------------------------------------------------------------------

use std::{
    error::Error,
    fmt,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use super::rules::ReloadableCwtRules;
use super::service::CwtLanguageService;

pub struct RhoiScribeRuntime {
    cwt_rules: RwLock<ReloadableCwtRules>,
    cwt_language: CwtLanguageService,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RhoiScribeRuntimeError {
    CwtRulesLockPoisoned,
}

impl RhoiScribeRuntime {
    pub fn new() -> Self {
        Self {
            cwt_rules: RwLock::new(ReloadableCwtRules::new()),
            cwt_language: CwtLanguageService::new(),
        }
    }

    pub fn read_cwt_rules(
        &self,
    ) -> Result<RwLockReadGuard<'_, ReloadableCwtRules>, RhoiScribeRuntimeError> {
        self.cwt_rules
            .read()
            .map_err(|_| RhoiScribeRuntimeError::CwtRulesLockPoisoned)
    }

    pub fn write_cwt_rules(
        &self,
    ) -> Result<RwLockWriteGuard<'_, ReloadableCwtRules>, RhoiScribeRuntimeError> {
        self.cwt_rules
            .write()
            .map_err(|_| RhoiScribeRuntimeError::CwtRulesLockPoisoned)
    }

    pub fn cwt_language(&self) -> &CwtLanguageService {
        &self.cwt_language
    }
}

impl Default for RhoiScribeRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for RhoiScribeRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RhoiScribeRuntime")
            .field("cwt_rules", &"RwLock<ReloadableCwtRules>")
            .field("cwt_language", &self.cwt_language)
            .finish()
    }
}

impl fmt::Display for RhoiScribeRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RhoiScribeRuntimeError::CwtRulesLockPoisoned => {
                write!(formatter, "CWT rules runtime lock is poisoned")
            }
        }
    }
}

impl Error for RhoiScribeRuntimeError {}
