import{S as f,i as $,s as u,y as s,z as i,A as p,g as m,d as c,B as l,I as d}from"../chunks/index.6f0c0f63.mjs";import{p as g}from"../chunks/api.15f7bc3a.mjs";import{D as _}from"../chunks/DashboardPage.c4ea9bf6.mjs";import{I as y}from"../chunks/Inventory.050ee8ca.mjs";function v(r){let t,n;return t=new y({props:{playerId:r[0].id,displayName:r[0].display_name}}),{c(){s(t.$$.fragment)},l(e){i(t.$$.fragment,e)},m(e,a){p(t,e,a),n=!0},p(e,a){const o={};a&1&&(o.playerId=e[0].id),a&1&&(o.displayName=e[0].display_name),t.$set(o)},i(e){n||(m(t.$$.fragment,e),n=!0)},o(e){c(t.$$.fragment,e),n=!1},d(e){l(t,e)}}}function h(r){let t,n;return t=new _({props:{title:"Inventory",text:"Click an inventory category to view its contents",$$slots:{default:[v]},$$scope:{ctx:r}}}),{c(){s(t.$$.fragment)},l(e){i(t.$$.fragment,e)},m(e,a){p(t,e,a),n=!0},p(e,[a]){const o={};a&3&&(o.$$scope={dirty:a,ctx:e}),t.$set(o)},i(e){n||(m(t.$$.fragment,e),n=!0)},o(e){c(t.$$.fragment,e),n=!1},d(e){l(t,e)}}}function I(r,t,n){let e;return d(r,g,a=>n(0,e=a)),[e]}class N extends f{constructor(t){super(),$(this,t,I,h,u,{})}}export{N as default};
